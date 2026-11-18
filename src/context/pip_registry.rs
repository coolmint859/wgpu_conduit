use std::{cell::RefCell, collections::{HashMap, HashSet}};
use async_store::{HoldPolicy, ResourceHandler, Task, TaskConfig};

use crate::{context::*, blueprints::*};

/// Keeps track of and validates requests for render/compute pipelines
pub(crate) struct PipelineRegistry {
    gpu: GpuHandle,
    pip_handles: ResourceHandler<PipelineId, PipelineHandle, String>,
    shader_handles: ResourceHandler<ShaderId, wgpu::ShaderModule, String>,
    pip_bps: HashMap<PipelineId, Pipeline>,
    deferred: HashMap<PipelineId, Pipeline>,
    valid_pips: RefCell<HashSet<PipelineId>>
}

impl PipelineRegistry {
    pub fn new(gpu: GpuHandle) -> Self {
        Self {
            gpu,
            pip_handles: ResourceHandler::new(),
            shader_handles: ResourceHandler::new(),
            pip_bps: HashMap::new(),
            deferred: HashMap::new(),
            valid_pips: RefCell::new(HashSet::new())
        }
    }

    /// Request a new pipeline
    pub fn request_pip(&mut self, blueprint: Pipeline, bgs: &BindGroupRegistry, global_hp: HoldPolicy) -> PipelineId {
        let id = blueprint.create_id();

        // pipeline ids are content-addressable, so its possible the same blueprint was used
        if self.pip_handles.contains(&id) { return id };

        self.try_request(&id, blueprint, bgs, global_hp);

        id
    }

    /// Request a new shader module
    pub fn request_shader(&mut self, shader: &ShaderProgram, global_hp: HoldPolicy) -> ShaderId {
        let id = shader.create_id();

        let policy = shader.hold_policy.unwrap_or(global_hp);
        let config = TaskConfig::new(policy);

        let gpu = self.gpu.clone();
        let blueprint = shader.clone();

        self.shader_handles.request(id.clone(), Task::io_bound(config, async move {
            gpu.create_shader_module(&blueprint)
        }));

        id
    }

    /// Sync the registry and process deferred groups
    pub fn sync(&mut self, bgs: &BindGroupRegistry, global_hp: HoldPolicy) {
        self.pip_handles.sync();
        self.shader_handles.sync();

        let pending_bgs = std::mem::take(&mut self.deferred);
        for (id, blueprint) in pending_bgs {
            self.try_request(&id, blueprint, bgs, global_hp);
        }
    }

    /// Remove a bind group from the registry
    pub fn remove_pipeline(&mut self, pip_id: &PipelineId, bgs: &mut BindGroupRegistry) {
        if let Some(blueprint) = self.pip_bps.remove(pip_id) {
            for layout_id in blueprint.bg_layouts().iter() {
                bgs.check_dec_bgl(layout_id);
            }
        }

        self.pip_handles.remove(pip_id);
        self.deferred.remove(pip_id);
        self.valid_pips.borrow_mut().remove(pip_id);
    }

    /// Remove a shader from the pipeline registry.
    /// 
    /// This also invalidates all pipelines that used the shader.
    pub fn remove_shader(&mut self, shader_id: &ShaderId) {
        self.shader_handles.remove(shader_id);

        let mut invalid_pips = HashSet::new();
        for (pip_id, pip_bp) in &self.pip_bps {
            if pip_bp.contains_shader(shader_id) {
                invalid_pips.insert(*pip_id);
            }
        }

        for pip_id in &invalid_pips {
            self.valid_pips.borrow_mut().remove(pip_id);
        }
    }

    /// Validates and returns a pipeline handle corresponding to the id if it passes
    pub fn get_pipeline(&self, pip_id: &PipelineId, bgs: &BindGroupRegistry) -> Option<&PipelineHandle> {
        if self.valid_pips.borrow().contains(pip_id) {
            return self.pip_handles.get(pip_id);
        }

        match self.pip_bps.get(pip_id)? {
            Pipeline::Render(render_bp) => {
                self.resolve_render_deps(render_bp, bgs)?;
            }
            Pipeline::Compute(compute_bp) => {
                self.resolve_compute_deps(compute_bp, bgs)?;
            }
        }

        self.valid_pips.borrow_mut().insert(*pip_id);
        self.pip_handles.get(pip_id)
    }

    /// Try requesting a new pipeline tied to the provided id.
    fn try_request(
        &mut self,
        pip_id: &PipelineId,
        blueprint: Pipeline, 
        bgs: &BindGroupRegistry,
        global_hp: HoldPolicy
    ) {
        match &blueprint {
            Pipeline::Render(render_bp) => {
                if let Some(deps) = self.resolve_render_deps(render_bp, bgs) {
                    self.pip_bps.insert(*pip_id, blueprint.clone());
                    for layout_id in render_bp.bg_layouts.iter() {
                        bgs.check_inc_bgl(layout_id);
                    }
                    
                    let gpu = self.gpu.clone();
                    let pip_bp = render_bp.clone();
                    let policy = render_bp.hold_policy.unwrap_or(global_hp);
                    let task_config = TaskConfig::new(policy);

                    let render_task = Task::io_bound(task_config, async move {
                        gpu.create_render_pipeline(pip_bp, deps)
                    });

                    self.pip_handles.request(*pip_id, render_task);
                } else {
                    self.deferred.insert(*pip_id, blueprint.clone());
                }
            }
            Pipeline::Compute(compute_bp) => {
                if let Some(deps) = self.resolve_compute_deps(compute_bp, bgs) {
                    self.pip_bps.insert(*pip_id, blueprint.clone());
                    for layout_id in compute_bp.bg_layouts.iter() {
                        bgs.check_inc_bgl(layout_id);
                    }

                    let gpu = self.gpu.clone();
                    let pip_bp = compute_bp.clone();
                    let policy = compute_bp.hold_policy.unwrap_or(global_hp);
                    let task_config = TaskConfig::new(policy);

                    let render_task = Task::io_bound(task_config, async move {
                        gpu.create_compute_pipeline(pip_bp, deps)
                    });

                    self.pip_handles.request(*pip_id, render_task);
                } else {
                    self.deferred.insert(*pip_id, blueprint);
                }
            }
        }
    }

    /// Resolves the dependencies of a render pipeline given it's `blueprint` and a `BindGroupRegistry`
    ///
    /// Returns `None` if at least one dependency doesn't resolve.
    fn resolve_render_deps(&self, blueprint: &PipelineBlueprint<Render>, bgs: &BindGroupRegistry) -> Option<RenderPipelineDependencies> {        
        let layouts = self.resolve_layouts(&blueprint.bg_layouts, bgs)?;

        let v_shader = &blueprint.ty.vertex_shader;
        let vertex_shader = ResolvedShaderProgram {
            module: self.shader_handles.get(&v_shader.id)?.clone(),
            entry: v_shader.entry.clone()
        };

        let fragment_shader = blueprint.ty.fragment_shader
            .clone()
            .map_or(None, |f_shader| {
                Some(ResolvedShaderProgram {
                    module: self.shader_handles.get(&f_shader.id)?.clone(),
                    entry: f_shader.entry
                })
            });

        Some(RenderPipelineDependencies {
            layouts,
            vertex_shader,
            fragment_shader
        })
    }

    /// Resolves the dependencies of a compute pipeline given it's `blueprint` and a `BindGroupRegistry`.
    /// 
    /// Returns `None` if at least one dependency doesn't resolve.
    fn resolve_compute_deps(&self, blueprint: &PipelineBlueprint<Compute>, bgs: &BindGroupRegistry) -> Option<ComputePipelineDependencies> {
        let layouts = self.resolve_layouts(&blueprint.bg_layouts, bgs)?;

        let compute_shader = &blueprint.ty.shader;
        let shader = ResolvedShaderProgram {
            module: self.shader_handles.get(&compute_shader.id)?.clone(),
            entry: compute_shader.entry.clone()
        };

        Some(ComputePipelineDependencies { layouts, shader })
    }

    /// Returns the resolved `wgpu::BindGroupLayout`s that map to the provided set of `LayoutId`s, and `None` if at least one layout doesn't resolve.
    fn resolve_layouts(&self, layouts: &Vec<LayoutId>, bgs: &BindGroupRegistry) -> Option<Vec<wgpu::BindGroupLayout>> {
        let resolved: Vec<_> = layouts
            .iter()
            .map_while(|id| {
                Some(bgs.get_layout(id)?.clone())
            })
            .collect();

        Some(resolved)
    }
}