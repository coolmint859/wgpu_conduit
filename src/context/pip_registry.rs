use std::{cell::RefCell, collections::{HashMap, HashSet}, unreachable};
use async_store::{HoldPolicy, ResourceHandler, Task, TaskConfig};

use crate::{context::*, blueprints::*};

pub enum ShaderDependency {
    Render{
        vertex: ResolvedShaderProgram,
        fragment: Option<ResolvedShaderProgram>,
    },
    Compute {
        shader: ResolvedShaderProgram,
    }
}

/// The resolved dependencies of a bind group
pub(crate) struct PipelineDependencies {
    pub bg_layouts: Vec<wgpu::BindGroupLayout>,
    pub shaders: ShaderDependency
}

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
    pub fn request_shader(&mut self, blueprint: &ShaderBlueprint, global_hp: HoldPolicy) -> ShaderId {
        let id = blueprint.create_id();

        let policy = blueprint.hold_policy.unwrap_or(global_hp);
        let config = TaskConfig::new(policy);

        let gpu = self.gpu.clone();
        let blueprint = blueprint.clone();

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
        for (id, pip_def) in &pending_bgs {
            self.try_request(id, pip_def.clone(), bgs, global_hp);
        }
    }

    /// Remove a bind group from the registry
    pub fn remove(&mut self, pip_id: &PipelineId, bgs: &mut BindGroupRegistry) {
        if let Some(blueprint) = self.pip_bps.remove(pip_id) {
            for layout_id in blueprint.bg_layouts().iter() {
                bgs.check_dec_bgl(layout_id);
            }
        }

        self.pip_handles.remove(pip_id);
        self.deferred.remove(pip_id);
        self.invalidate(pip_id);
    }

    /// Validates and returns a pipeline handle corresponding to the id if it passes
    pub fn validate(&self, pip_id: &PipelineId, bgs: &BindGroupRegistry) -> Option<&PipelineHandle> {
        if self.valid_pips.borrow().contains(pip_id) {
            return self.pip_handles.get(pip_id);
        }

        let blueprint = self.pip_bps.get(pip_id)?;
        self.resolve_dependencies(blueprint, bgs)?;

        self.valid_pips.borrow_mut().insert(*pip_id);
        self.pip_handles.get(pip_id)
    }

    /// Invalidate a pipeline. This prevents it from being used in subsequent gpu commands
    pub fn invalidate(&self, pip_id: &PipelineId) {
        self.valid_pips.borrow_mut().remove(pip_id);
    }

    /// Try requesting a new pipeline tied to the provided id.
    fn try_request(
        &mut self,
        pip_id: &PipelineId,
        blueprint: Pipeline, 
        bgs: &BindGroupRegistry,
        global_hp: HoldPolicy
    ) {
        if let Some(deps) = self.resolve_dependencies(&blueprint, bgs) {
            self.deferred.remove(pip_id);

            if !self.pip_bps.contains_key(pip_id) {
                self.pip_bps.insert(*pip_id, blueprint.clone());
            }

            for layout_id in blueprint.bg_layouts().iter() {
                bgs.check_inc_bgl(layout_id);
            }

            let gpu = self.gpu.clone();
            match blueprint {
                Pipeline::Render(pip_bp) => {
                    let policy = pip_bp.hold_policy.unwrap_or(global_hp);
                    let task_config = TaskConfig::new(policy);

                    // This is a code smell that I'm aware of. Eventually I want to refactor the pipeline and shader creation system to reduce issues like this.
                    let (vertex_shader, fragment_shader) = match deps.shaders {
                        ShaderDependency::Render { vertex, fragment } => {
                            (vertex, fragment)
                        },
                        ShaderDependency::Compute { .. } => { unreachable!("Shouldn't have compute pipeline dependencies for a render pipeline!")}
                    };

                    let r_pip_task = Task::io_bound(task_config, async move {
                        gpu.create_render_pipeline(
                            pip_bp,
                            vertex_shader,
                            fragment_shader,
                            deps.bg_layouts
                        )
                    });

                    self.pip_handles.request(*pip_id, r_pip_task);
                },
                Pipeline::Compute(pip_bp) => {
                    let policy = pip_bp.hold_policy.unwrap_or(global_hp);
                    let task_config = TaskConfig::new(policy);

                    // This is a code smell that I'm aware of. Eventually I want to refactor the pipeline and shader creation system to reduce issues like this.
                    let compute_shader = match deps.shaders {
                        ShaderDependency::Compute { shader } => shader,
                        ShaderDependency::Render {..} => { unreachable!("Shouldn't have render pipeline dependencies for a compute pipeline!") },
                    };

                    let c_pip_task = Task::io_bound(task_config, async move {
                        gpu.create_compute_pipeline(
                            pip_bp, 
                            compute_shader,
                            deps.bg_layouts
                        )
                    });

                    self.pip_handles.request(*pip_id, c_pip_task);
                }
            };
        } else {
            self.deferred.insert(*pip_id, blueprint);
        }
    }

    /// Resolves the dependencies of a pipeline given it's `blueprint` and a `BindGroupRegistry`
    fn resolve_dependencies(&self, blueprint: &Pipeline, bgs: &BindGroupRegistry) -> Option<PipelineDependencies> {
        let mut bg_layouts = Vec::new();
        for bgl_id in blueprint.bg_layouts().iter() {
            if let Some(layout) =  bgs.layout_handles.get(bgl_id).cloned() {
                bg_layouts.push((*layout).clone())
            } else {
                return None
            }
        }

        match blueprint {
            Pipeline::Render(pip) => {
                let v_shader = pip.ty.vertex_shader;
                let module = self.shader_handles.get(&v_shader.id)?.clone();
                let vertex = ResolvedShaderProgram {
                    module,
                    entry: v_shader.entry
                };

                let fragment = match pip.ty.fragment_shader {
                    Some(f_shader) => {
                        let module = self.shader_handles.get(&f_shader.id)?.clone();
                        Some(ResolvedShaderProgram {
                            module,
                            entry: f_shader.entry
                        })
                    },
                    None => None,
                };

                Some(PipelineDependencies {
                    bg_layouts,
                    shaders: ShaderDependency::Render { vertex, fragment }
                })
            },
            Pipeline::Compute(pip) => {
                let compute_shader = pip.ty.shader;
                let module = self.shader_handles.get(&compute_shader.id)?.clone();
                let shader = ResolvedShaderProgram {
                    module,
                    entry: compute_shader.entry
                };

                Some(PipelineDependencies {
                    bg_layouts,
                    shaders: ShaderDependency::Compute { shader }
                })
            }
        }
    }
}