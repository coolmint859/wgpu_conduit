use std::hash::{DefaultHasher, Hash, Hasher};

use async_store::HoldPolicy;
use uuid::Uuid;

use crate::{context::*, blueprints::{VertexBufferLayout, VertexLayout}};

/// The method of obtaining the source code for a shader module
#[derive(Clone, Debug, Hash)]
pub enum ShaderSource {
    /// Shader code is loaded in dynamically from disk
    OnDisk(&'static str),
    /// Shader code is embedded in the binary of the driver program
    /// 
    /// Works best with `include_str!()`
    Embedded(&'static str)
}

/// Represents a handle to a render/compute pipeline
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipelineHandle {
    Render(wgpu::RenderPipeline),
    Compute(wgpu::ComputePipeline)
} 

impl PipelineHandle {
    /// Get the render pipeline handle if this handle is the Render variant
    pub fn to_render(&self) -> Option<wgpu::RenderPipeline> {
        match self {
            PipelineHandle::Render(handle) => Some(handle.clone()),
            PipelineHandle::Compute(_) => None
        }
    }

    /// Get the compute pipeline handle if this handle is the Compute variant
    pub fn to_compute(&self) -> Option<wgpu::ComputePipeline> {
        match self {
            PipelineHandle::Compute(handle) => Some(handle.clone()),
            PipelineHandle::Render(_) => None
        }
    }
}

/// Specifies the pipeline depth state configuration.
/// 
/// Default:
/// ```
/// DepthState {
///     format: wgpu::TextureFormat::Depth32Float,
///     compare_fn: wgpu::CompareFunction::Less,
///     write_enabled: true,
/// }
/// ```
pub struct DepthState {
    /// The format of the depth texture used.
    pub format: wgpu::TextureFormat,
    /// The compare function when evaulating the depth values in the depth texture.
    pub compare_fn: wgpu::CompareFunction,
    /// Determines whether the depth buffer can be written to.
    pub write_enabled: bool,
}

impl Default for DepthState {
    fn default() -> Self {
        Self {
            format: wgpu::TextureFormat::Depth32Float,
            compare_fn: wgpu::CompareFunction::Less,
            write_enabled: true,
        }
    }
}

/// Holds configuration state for a render pipeline blueprint
#[derive(Clone, Debug, Hash)]
pub struct Render {
    pub(crate) vs_main: &'static str, 
    pub(crate) fs_main: &'static str, 
    pub(crate) format: wgpu::TextureFormat,
    pub(crate) vertex_layouts: Vec<VertexBufferLayout>,
    pub(crate) color_targets: Vec<Option<wgpu::ColorTargetState>>,
    pub(crate) depth_stencil: Option<wgpu::DepthStencilState>,
    pub(crate) primitive_state: wgpu::PrimitiveState,
}

impl Render {
    pub fn new(vs_main: &'static str, fs_main: &'static str) -> Self {
        Self {
            vs_main,
            fs_main,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            vertex_layouts: Vec::new(),
            depth_stencil: None,
            color_targets: Vec::new(),
            primitive_state: wgpu::PrimitiveState::default()
        }
    }
}

/// Holds configurations state for a compute pipeline creation
#[derive(Clone, Copy, Debug, Hash)]
pub struct Compute {
    pub(crate) main: &'static str,
}

impl Compute {
    pub fn new(main: &'static str) -> Self {
        Self { main }
    }
}

/// The type of gpu pipeline. This is used internally by the context to create the underlying wgpu pipelines
#[derive(Clone, Debug)]
pub enum Pipeline {
    Render(PipelineBlueprint<Render>),
    Compute(PipelineBlueprint<Compute>)
}

impl Pipeline {
    /// Get the bind group layouts associated with the pipeline
    pub fn bg_layouts(&self) -> &Vec<LayoutId> {
        return match self {
            Pipeline::Render(pip) => {
                &pip.bg_layouts
            }
            Pipeline::Compute(pip) => {
                &pip.bg_layouts
            }
        }
    }
}

impl From<PipelineBlueprint<Render>> for Pipeline {
    fn from(pip: PipelineBlueprint<Render>) -> Self {
        Self::Render(pip)
    }
}

impl From<PipelineBlueprint<Compute>> for Pipeline {
    fn from(pip: PipelineBlueprint<Compute>) -> Self {
        Self::Compute(pip)
    }
}

impl GpuResource for Pipeline {
    type Id = PipelineId;

    fn create_id(&self) -> Self::Id {
        return match self {
            Pipeline::Compute(pip) => pip.create_id(),
            Pipeline::Render(pip) => pip.create_id(),
        }
    }
}

/// A blueprint for constructing render and compute pipelines
#[derive(Clone, Debug, Hash)]
pub struct PipelineBlueprint<T: Hash> {
    pub label: String,
    pub bg_layouts: Vec<LayoutId>,
    pub shader: ShaderSource,
    pub hold_policy: Option<HoldPolicy>,
    pub ty: T,
}

impl<T: Hash> PipelineBlueprint<T> {
    /// Set the label for gpu profiling of the resultant render pipeline
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    /// Add bind group layouts to the pipeline
    pub fn with_bg_layouts(mut self, layouts: &[LayoutId]) -> Self {
        self.bg_layouts.extend_from_slice(layouts);
        self
    }

    /// Set the hold policy for this compute pipeline. This overrides the global policy.
    pub fn with_hold_policy(mut self, policy: HoldPolicy) -> Self {
        self.hold_policy = Some(policy);
        self
    }
}

impl<T: Hash> GpuResource for PipelineBlueprint<T> {
    type Id = PipelineId;

    fn create_id(&self) -> Self::Id {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        return PipelineId(Uuid::from_u128(hasher.finish() as u128));
    }
}

impl PipelineBlueprint<Render> {
    /// Create a new render pipeline.
    pub fn as_render(shader: ShaderSource) -> Self {
        Self {
            label: "render_pipeline".to_string(),
            bg_layouts: Vec::new(),
            shader,
            hold_policy: None,
            ty: Render::new("vs_main", "fs_main")
        }
    }

    /// Set the entry points of the render shader for the render pipeline
    pub fn with_entry_points(mut self, vs_main: &'static str, fs_main: &'static str) -> Self {
        self.ty.vs_main = vs_main;
        self.ty.fs_main = fs_main;
        self
    }

    /// Set the output format of the render pipeline. This must match the target format for the corresponding render pass
    pub fn with_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.ty.format = format;
        self
    }

    /// Add a vertex layout to the rendering pipeline. A corresponding buffer must be added to the render pass
    pub fn with_vertex_layout<V: VertexLayout>(mut self) -> Self {
        self.ty.vertex_layouts.push(V::into_layout());
        self
    }

    /// Set the depth stencil state for the pipeline
    pub fn with_depth_state(mut self, state: Option<wgpu::DepthStencilState>) -> Self {
        self.ty.depth_stencil = state;
        self
    }

    /// Add a color target state to the pipeline.
    pub fn with_color_state(mut self, state: Option<wgpu::ColorTargetState>) -> Self {
        self.ty.color_targets.push(state);
        self
    }

    /// Configure the pipeline to interpret vertices with the provided topology
    pub fn with_topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
        self.ty.primitive_state.topology = topology;
        self
    }

    /// Configure the pipeline to consider front faces to be in clockwise winding order.
    pub fn with_cw_winding(mut self) -> Self {
        self.ty.primitive_state.front_face = wgpu::FrontFace::Cw;
        self
    }

    /// Configure the pipeline to consider front faces to be in counter-clockwise winding order.
    pub fn with_ccw_winding(mut self) -> Self {
        self.ty.primitive_state.front_face = wgpu::FrontFace::Ccw;
        self
    }

    /// Configure the pipeline to cull back faces
    pub fn with_back_culling(mut self) -> Self {
        self.ty.primitive_state.cull_mode = Some(wgpu::Face::Back);
        self
    }

    /// Configure the pipeline to cull front faces
    pub fn with_front_culling(mut self) -> Self {
        self.ty.primitive_state.cull_mode = Some(wgpu::Face::Front);
        self
    }
}

impl PipelineBlueprint<Compute> {
    /// Create a new compute pipeline
    pub fn as_compute(shader: ShaderSource) -> Self {
        Self {
            label: "compute_pipeline".to_string(),
            bg_layouts: Vec::new(),
            shader,
            hold_policy: None,
            ty: Compute::new("cs_main")
        }
    }

    /// Set the entry point of the compute shader for the compute pipeline
    pub fn with_entry_point(mut self, main: &'static str) -> Self {
        self.ty.main = main;
        self
    }
}
