use std::{cell::{Cell, RefCell}, collections::{HashMap, HashSet}, ops::Deref};
use async_store::{HoldPolicy, ResourceHandler, Task, TaskConfig};

use crate::{gpu::GpuHandle, context::*, blueprints::*};

/// A handle to a bind group and associated layout
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindGroupHandle {
    pub bind_group: wgpu::BindGroup,
    pub layout_id: LayoutId
}

impl Deref for BindGroupHandle {
    type Target = wgpu::BindGroup;

    fn deref(&self) -> &Self::Target {
        &self.bind_group
    }
}

/// A handle to a bind group layout
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindGroupLayoutHandle {
    pub ref_count: Cell<usize>,
    pub layout: wgpu::BindGroupLayout,
}

impl Deref for BindGroupLayoutHandle {
    type Target = wgpu::BindGroupLayout;

    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

/// Keeps track of and validates requests for bind groups
pub(crate) struct BindGroupRegistry {
    gpu: GpuHandle,
    /// The handles to bind groups
    bg_handles: ResourceHandler<BindGroupId, BindGroupHandle, String>,
    /// The handles to bind group layouts
    layout_handles: ResourceHandler<LayoutId, BindGroupLayoutHandle, String>,
    /// The blueprints for all requested bind groups
    blueprints: HashMap<BindGroupId, BindGroupBlueprint>,
    /// The blueprints for bind groups with pending dependencies
    deferred: HashMap<BindGroupId, BindGroupBlueprint>,
    /// The set of all known valid bind groups
    valid_bgs: RefCell<HashSet<BindGroupId>>,
}

impl BindGroupRegistry {
    pub fn new(gpu: GpuHandle) -> Self {
        Self {
            gpu,
            bg_handles: ResourceHandler::new(),
            layout_handles: ResourceHandler::new(),
            blueprints: HashMap::new(),
            deferred: HashMap::new(),
            valid_bgs: RefCell::new(HashSet::new()),
        }
    }

    /// Request a new bind group layout
    pub fn request_layout(&mut self, id: &LayoutId, blueprint: &BindGroupBlueprint) {
        if self.layout_handles.contains(id) { return; };

        let gpu = self.gpu.clone();
        let blueprint = blueprint.clone();

        let task_config = TaskConfig::new(HoldPolicy::Indefinite);

        let layout_task = Task::io_bound(task_config, async move {
            gpu.create_bg_layout(blueprint).map(|layout| {
                BindGroupLayoutHandle { layout, ref_count: Cell::new(0) }
            })
        });

        self.layout_handles.request(*id, layout_task);
    }

    /// Request a new bind group
    pub fn request_bg(&mut self, blueprint: BindGroupBlueprint, resources: &GpuResources, global_hp: HoldPolicy) -> BindGroupIdPair {
        let bg_id = blueprint.create_id();
        let layout_id = blueprint.layout.create_id();
        
        self.try_request_bg(blueprint, resources, global_hp, &bg_id);

        BindGroupIdPair {
            id: bg_id,
            layout_id
        }
    }

    /// sync the registry and process deferred groups
    pub fn sync(&mut self, resources: &GpuResources, global_hp: HoldPolicy) {
        self.layout_handles.sync();
        self.bg_handles.sync();

        let deferred = std::mem::take(&mut self.deferred);
        for (bg_id, blueprint) in deferred {
            self.try_request_bg(blueprint, resources, global_hp, &bg_id);
        }
    }

    /// remove a bind group from the registry
    pub fn remove_bg(&mut self, bg_id: &BindGroupId) {
        if let Some(blueprint) = self.blueprints.remove(bg_id) {
            self.check_dec_bgl(&blueprint.layout.create_id());
        }
        self.deferred.remove(bg_id);
        self.bg_handles.remove(bg_id);
    }

    /// Returns a refernce to the `wgpu::BindGroupLayout` matching the provided `LayoutId`.
    /// 
    /// This marks the layout as accessed. However since layout lifetimes are based on reference counting and not TTL, this doesn't matter much.
    pub fn get_layout(&self, layout_id: &LayoutId) -> Option<&wgpu::BindGroupLayout> {
        self.layout_handles
            .get(layout_id)
            .map_or(None, |handle| {
                Some(&handle.layout)
            })
    }

    /// Validates and returns a bind group corresponding to the id if it passes
    pub fn get_bindgroup(&self, bg_id: &BindGroupId, resources: &GpuResources) -> Option<BindGroupHandle> {
        if self.valid_bgs.borrow().contains(bg_id) {
            self.mark_dependencies_accessed(bg_id, resources);
            return self.bg_handles.get(bg_id).cloned();
        }

        let blueprint = self.blueprints.get(bg_id)?.clone();
        self.resolve_dependencies(&blueprint, &resources)?;

        self.valid_bgs.borrow_mut().insert(*bg_id);
        self.bg_handles.get(bg_id).cloned()
    }

    /// Invalidates all bind groups that reference the provided `BufferId`.
    pub fn invalidate_matching_buffer(&mut self, buf_id: &BufferId) {
        let mut invalid_bgs = HashSet::new();
        for (bd_id, bg_blueprint) in &self.blueprints {
            for entry in &bg_blueprint.bindings {
                if entry.target.matches_buffer(buf_id) {
                    invalid_bgs.insert(*bd_id);
                    continue;
                }
            }
        }

        for bg_id in &invalid_bgs {
            self.valid_bgs.borrow_mut().remove(bg_id);
        }
    }

    /// Invalidates all bind groups that reference the provided `TextureId`.
    pub fn invalidate_matching_texture(&mut self, tex_id: &TextureId) {
        let mut invalid_bgs = HashSet::new();
        for (bd_id, bg_blueprint) in &self.blueprints {
            for entry in &bg_blueprint.bindings {
                if entry.target.matches_texture(tex_id) {
                    invalid_bgs.insert(*bd_id);
                    continue;
                }
            }
        }

        for bg_id in &invalid_bgs {
            self.valid_bgs.borrow_mut().remove(bg_id);
        }
    }

    /// Invalidates all bind groups that reference the provided `SamplerId`.
    pub fn invalidate_matching_sampler(&mut self, samp_id: &SamplerId) {
        let mut invalid_bgs = HashSet::new();
        for (bd_id, bg_blueprint) in &self.blueprints {
            for entry in &bg_blueprint.bindings {
                if entry.target.matches_sampler(samp_id) {
                    invalid_bgs.insert(*bd_id);
                    continue;
                }
            }
        }

        for bg_id in &invalid_bgs {
            self.valid_bgs.borrow_mut().remove(bg_id);
        }
    }

    /// Decrement the reference count of a bind group layout, if exists. 
    /// If the count hits 0, this will remove it from the registry
    pub fn check_dec_bgl(&mut self, bgl_id: &LayoutId) {
        let mut should_remove = false;
        self.layout_handles.apply_if_ready(bgl_id, |layout | {
            let next = layout.ref_count.get().saturating_sub(1);
            layout.ref_count.set(next);
            should_remove = layout.ref_count.get() == 0;
        });

        if should_remove {
            self.layout_handles.remove(bgl_id);
        }
    }

    /// Increment the reference count of a bind group layout, if exists.
    pub fn check_inc_bgl(&self, bgl_id: &LayoutId) {
        self.layout_handles.apply_if_ready(bgl_id, |layout | {
            let next = layout.ref_count.get().saturating_add(1);
            layout.ref_count.set(next);
        });
    }

    /// Try requesting a new bind group and layout tied to the provided ids.
    fn try_request_bg(
        &mut self, 
        blueprint: BindGroupBlueprint,
        resources: &GpuResources,
        global_hp: HoldPolicy,
        bg_id: &BindGroupId,
    ) {
        let layout_id = blueprint.layout.create_id();
        if let Some(deps) = self.resolve_dependencies(&blueprint, resources) {
            self.blueprints.insert(*bg_id, blueprint.clone());
            
            self.check_inc_bgl(&layout_id);

            let gpu = self.gpu.clone();
            let policy = blueprint.hold_policy.unwrap_or(global_hp);
            let task_config = TaskConfig::new(policy);

            let bind_group_task = Task::io_bound(task_config, async move {
                gpu.create_bind_group(blueprint, deps).map(|bind_group| {
                    BindGroupHandle { bind_group, layout_id }
                })
            });

            self.bg_handles.request(*bg_id, bind_group_task);
        } else {
            self.request_layout(&layout_id, &blueprint);
            self.deferred.insert(*bg_id, blueprint.clone());
        }
    }

    /// Mark the bind group and dependencies as accessed.
    pub fn mark_bg_accessed(&self, bg_id: &BindGroupId, resources: &GpuResources) {
        self.bg_handles.mark_accessed(bg_id);
        self.mark_dependencies_accessed(bg_id, resources);
    }

    /// Returns `true` if a bind group mapped to the provided id exists or is pending completion, `false` otherwise
    pub fn is_bg_requested(&self, bg_id: &BindGroupId) -> bool {
        self.bg_handles.is_requested(bg_id)
    }

    /// Fetches bind group dependencies if they are ready. If at least one dependency is not yet ready, None is returned
    fn resolve_dependencies(
        &self,
        blueprint: &BindGroupBlueprint,
        resources: &GpuResources,
    ) -> Option<BindGroupDependencies> {
        let targets = blueprint.bindings
            .iter()
            .map(|entry| entry.resolve(resources))
            .collect::<Option<Vec<_>>>()?;
        
        let layout_id = blueprint.layout.create_id();
        let layout = self.layout_handles.get(&layout_id)?.clone();
        return Some(BindGroupDependencies { layout, entries: targets })
    }

    /// Marks the dependencies of a bind group as accessed. This is called if a bind group was found to be valid, ensuring the dependencies stay alive for the next frame.
    fn mark_dependencies_accessed(&self, bg_id: &BindGroupId, resources: &GpuResources) {        
        if let Some(blueprint) = self.blueprints.get(bg_id) {
            for binding in &blueprint.bindings {
                binding.target.mark_accessed(resources);
            }
        }
    }
}