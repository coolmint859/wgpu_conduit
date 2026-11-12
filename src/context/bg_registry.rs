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

/// Contains the dependencies of a bind group. Used in bind group creation and to determine validity.
pub(crate) struct BindGroupDependencies {
    layout: BindGroupLayoutHandle,
    /// the resolved targets attached to a bind group, including the slot number
    targets: Vec<(ResolvedTarget, u32)>,
}

/// Keeps track of and validates requests for bind groups
pub(crate) struct BindGroupRegistry {
    gpu: GpuHandle,
    /// map of ids of bind group that have yet to pass request validation
    deferred: HashMap<BindGroupId, (LayoutId, BindGroupBlueprint)>,
    /// the set of bind groups that are known to be valid. 
    valid_bgs: RefCell<HashSet<BindGroupId>>,
    /// The handles to bind groups
    pub(crate) bg_handles: ResourceHandler<BindGroupId, BindGroupHandle, String>,
    /// The handles to bind group layouts
    pub(crate) layout_handles: ResourceHandler<LayoutId, BindGroupLayoutHandle, String>,
    /// maps bind groups to their blueprints
    pub(crate) bg_blueprints: HashMap<BindGroupId, BindGroupBlueprint>,
}

impl BindGroupRegistry {
    pub fn new(gpu: GpuHandle) -> Self {
        Self {
            gpu,
            bg_handles: ResourceHandler::new(),
            layout_handles: ResourceHandler::new(),
            bg_blueprints: HashMap::new(),
            deferred: HashMap::new(),
            valid_bgs: RefCell::new(HashSet::new())
        }
    }

    /// Request a new bind group layout
    pub fn request_layout(&mut self, id: &LayoutId, blueprint: &BindGroupBlueprint) {
        if self.layout_handles.contains(id) { return; };

        let gpu = self.gpu.clone();
        let builder = blueprint.clone();

        let task_config = TaskConfig::new(HoldPolicy::Indefinite);

        let layout_task = Task::io_bound(task_config, async move {
            gpu.create_bg_layout(builder)
                .and_then(|layout| Ok(BindGroupLayoutHandle { layout, ref_count: Cell::new(1)}))
        });

        self.layout_handles.request(*id, layout_task);
    }

    /// Request a new bind group
    pub fn request_bg(&mut self, blueprint: BindGroupBlueprint, resources: &GpuResources, global_hp: HoldPolicy) -> BindGroupIdPair {
        let bg_id = blueprint.create_id();
        let layout_id = blueprint.layout.create_id();
        
        self.try_request_bg(blueprint, resources, global_hp, &bg_id, &layout_id);

        BindGroupIdPair {
            id: bg_id,
            layout_id
        }
    }

    /// sync the registry and process deferred groups
    pub fn sync(&mut self, resources: &GpuResources, global_hp: HoldPolicy) {
        self.layout_handles.sync();
        self.bg_handles.sync();

        // println!("pending bind groups: {}", self.deffered.len());
        let pending_bgs = std::mem::take(&mut self.deferred);
        for (bg_id, (layout_id, blueprint)) in &pending_bgs {
            self.try_request_bg(blueprint.clone(), resources, global_hp, bg_id, layout_id);
        }
    }

    /// remove a bind group from the registry
    pub fn remove_bg(&mut self, bg_id: &BindGroupId) {
        if let Some(blueprint) = self.bg_blueprints.get(bg_id) {
            let layout_id = blueprint.layout.create_id();
            self.check_dec_bgl(&layout_id);
        }

        self.bg_handles.remove(bg_id);
        self.bg_blueprints.remove(bg_id);
        self.deferred.remove(bg_id);
        self.valid_bgs.borrow_mut().remove(bg_id);
    }

    /// Validates and returns a bind group corresponding to the id if it passes
    pub fn validate(&self, bg_id: &BindGroupId, resources: &GpuResources) -> Option<BindGroupHandle> {
        if self.valid_bgs.borrow().contains(bg_id) {
            self.mark_dependencies_accessed(bg_id, resources);
            return self.bg_handles.get(bg_id).cloned();
        }

        let layout_id = self.bg_handles.get(bg_id)?.layout_id;
        let blueprint = self.bg_blueprints.get(bg_id)?.clone();

        self.resolve_dependencies(&layout_id, &blueprint, &resources)?;

        self.valid_bgs.borrow_mut().insert(*bg_id);
        self.bg_handles.get(bg_id).cloned()
    }

    /// Invalidate a bind group. This prevents it from being used in subsequent gpu commands
    pub fn invalidate(&mut self, bg_id: &BindGroupId) {
        self.valid_bgs.borrow_mut().remove(bg_id);
    }

    /// Decrement the reference count of a bind group layout, if exists. 
    /// If the count hits 0, this will remove it from the registry
    pub fn check_dec_bgl(&mut self, bgl_id: &LayoutId) {
        let mut should_remove = false;
        self.layout_handles.apply_if_ready(bgl_id, |layout | {
            if layout.ref_count.get() > 0 {
                layout.ref_count.set(layout.ref_count.get() - 1);
            }
            should_remove = layout.ref_count.get() == 0;
        });

        if should_remove {
            self.layout_handles.remove(bgl_id);
        }
    }

    /// Increment the reference count of a bind group layout, if exists.
    pub fn check_inc_bgl(&self, bgl_id: &LayoutId) {
        self.layout_handles.apply_if_ready(bgl_id, |layout | {
            layout.ref_count.set(layout.ref_count.get() + 1);
        });
    }

    /// Try requesting a new bind group and layout tied to the provided ids.
    fn try_request_bg(
        &mut self, 
        blueprint: BindGroupBlueprint,
        resources: &GpuResources,
        global_hp: HoldPolicy,
        bg_id: &BindGroupId, 
        layout_id: &LayoutId
    ) {
        if let Some(deps) = self.resolve_dependencies(&layout_id, &blueprint, resources) {
            self.deferred.remove(bg_id);

            if !self.bg_blueprints.contains_key(&bg_id) {
                self.bg_blueprints.insert(*bg_id, blueprint.clone());
            }

            self.check_inc_bgl(&layout_id);

            let gpu = self.gpu.clone();
            let layout_id = layout_id.clone();
            let policy = blueprint.hold_policy.unwrap_or(global_hp);
            let task_config = TaskConfig::new(policy);

            let bind_group_task = Task::io_bound(task_config, async move {
                let mut entries = Vec::new();

                for (target, slot) in &deps.targets {
                    entries.push(wgpu::BindGroupEntry {
                        binding: *slot,
                        resource: target.to_wgpu_binding()
                    });
                }

                gpu.create_bind_group(blueprint, entries, deps.layout)
                    .and_then(|bind_group| Ok(BindGroupHandle { 
                        bind_group, 
                        layout_id 
                    }))
            });

            self.bg_handles.request(*bg_id, bind_group_task);
        } else {
            self.request_layout(layout_id, &blueprint);
            self.deferred.insert(*bg_id, (*layout_id, blueprint.clone()));
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
        layout_id: &LayoutId,
        blueprint: &BindGroupBlueprint,
        resources: &GpuResources,
    ) -> Option<BindGroupDependencies> {
        let mut targets = Vec::new();
        for binding in &blueprint.bindings {
            let target = binding.target.resolve(resources)?;
            targets.push((target, binding.slot));
        }
        
        let layout = self.layout_handles.get(layout_id)?.clone();
        return Some(BindGroupDependencies { layout, targets })
    }

    /// Marks the dependencies of a bind group as accessed. This is called if a bind group was found to be valid, ensuring the dependencies stay alive for the next frame.
    fn mark_dependencies_accessed(&self, bg_id: &BindGroupId, resources: &GpuResources) {
        if let Some(blueprint) = self.bg_blueprints.get(bg_id) {
            for binding in &blueprint.bindings {
                binding.target.mark_accessed(resources);
            }
        }
    }
}