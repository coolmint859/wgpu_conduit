use std::{hash::{DefaultHasher, Hash, Hasher}, num::NonZero};

use async_store::HoldPolicy;
use uuid::Uuid;

use crate::context::*;

/// The target ID for a bind group entry
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum BindingTarget {
    Buffer(BufferId),
    Texture(TextureId),
    Sampler(SamplerId),
}

impl BindingTarget {
    /// Resolves the target id into a `ResolvedTarget`, if available. Marks the resource as accessed.
    pub(crate) fn resolve(&self, resources: &GpuResources) -> Option<ResolvedTarget> {
        match &self {
            BindingTarget::Buffer(id) => {
                resources.buffers.get(id).cloned().map(ResolvedTarget::Buffer)
            }
            BindingTarget::Texture(id) => {
                resources.textures.get(id).cloned().map(ResolvedTarget::Texture)
            }
            BindingTarget::Sampler(id) => {
                resources.samplers.get(id).cloned().map(ResolvedTarget::Sampler)
            }
        }
    }

    /// Marks the resource mapped to the target id as accessed.
    pub(crate) fn mark_accessed(&self, resources: &GpuResources) {
        match &self {
            BindingTarget::Buffer(id) => resources.buffers.mark_accessed(id),
            BindingTarget::Texture(id) => resources.textures.mark_accessed(id),
            BindingTarget::Sampler(id) => resources.samplers.mark_accessed(id),
        }
    }

    /// Returns `true` if this binding target is a for a buffer binding and the provided id matches the target id, `false` otherwise.
    pub fn matches_buffer(&self, other: &BufferId) -> bool {
        match self {
            BindingTarget::Buffer(id) => other == id,
            _ => false
        }
    }

    /// Returns `true` if this binding target is for a texture binding and the provided id matches the target id, `false` otherwise.
    pub fn matches_texture(&self, other: &TextureId) -> bool {
        match self {
            BindingTarget::Texture(id) => other == id,
            _ => false
        }
    }

    /// Returns `true` if this binding target is for a sampler binding and the provided id matches the target id, `false` otherwise.
    pub fn matches_sampler(&self, other: &SamplerId) -> bool {
        match self {
            BindingTarget::Sampler(id) => other == id,
            _ => false
        }
    }
}

/// Represents gpu resources that can be used in a bind group
pub trait Bindable {
    /// Converts `self` into a `ResourceBinding`.
    fn into_binding(self) -> ResourceBinding;
}

/// Contains the binding information for a `BindGroupBlueprint` entry.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ResourceBinding {
    /// The target resource for the bind group entry.
    pub target: BindingTarget,
    /// the binding type of the resource entry.
    pub ty: wgpu::BindingType,
    /// the shader stage visibility of the resource.
    pub visibility: wgpu::ShaderStages,
}

impl Bindable for ResourceBinding {
    fn into_binding(self) -> ResourceBinding { self }
}

/// A resource entry into a bind group
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GroupEntry {
    /// The target resource id
    pub target: BindingTarget,
    /// The binding slot for the target resource
    pub slot: u32
}

impl GroupEntry {
    /// Resolves this `GroupEntry` into a `ResolvedEntry`
    pub(crate) fn resolve(&self, resources: &GpuResources) -> Option<ResolvedEntry> {
        self.target.resolve(resources).map(|rt| {
            ResolvedEntry { target: rt, slot: self.slot }
        })
    }
}

/// A blueprint for constructing a `wgpu::BindGroup`.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct BindGroupBlueprint {
    /// The label set for the `wgpu::BindGroup` used with gpu profiling
    pub(crate) label: String,
    /// The layout blueprint for the bind group.
    /// 
    /// A corresponding `wgpu::BindGroupLayout` will only be created if the id generated from the `LayoutBlueprint` has not yet been encountered by the context.
    pub(crate) layout: LayoutBlueprint,
    /// The set of bindings for the bind group
    pub(crate) bindings: Vec<GroupEntry>,
    /// The resource-specific hold policy for the bind group.
    /// 
    /// When set to `None`, the context's global policy for bind groups is used instead.
    pub(crate) hold_policy: Option<HoldPolicy>
}

impl BindGroupBlueprint {
    /// Create a new `BindGroupBlueprint`
    pub fn new() -> Self {
        Self {
            label: "bind_group".to_string(),
            layout: LayoutBlueprint::new(),
            bindings: Vec::new(),
            hold_policy: None,
        }
    }

    /// Set the label for gpu profiling of the resultant bind group
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    /// Set the hold policy for the `wgpu::BindGroup`. This overrides the global policy.
    pub fn with_hold_policy(mut self, policy: HoldPolicy) -> Self {
        self.hold_policy = Some(policy);
        self
    }

    /// Add an entry into the bind group
    pub fn with_entry(mut self, entry: impl Bindable) -> Self {
        self.add_entry(entry);
        self
    }

    /// Add an entry into the bind group
    pub fn add_entry(&mut self, entry: impl Bindable) {
        let binding = entry.into_binding();
        let slot = self.bindings.len() as u32;
        self.layout.add_entry(&binding);

        self.bindings.push(GroupEntry { 
            target: binding.target,
            slot
        });
    }
}

impl GpuResource for BindGroupBlueprint {
    type Id = BindGroupId;

    fn create_id(&self) -> Self::Id {
        BindGroupId(Uuid::new_v4())
    }
}

/// A blueprint for constucting a `wgpu::BindGroupLayout`. This is automatically generated by `BindGroupBlueprint`s.
/// 
/// Ids are generated based on contents. This ensures that matching `wgpu::BindGroupLayout`s are only created once.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct LayoutBlueprint {
    pub(crate) entries: Vec<wgpu::BindGroupLayoutEntry>
}

impl LayoutBlueprint {
    /// Create a new `LayoutBlueprint`
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Add an entry into the bind group layout
    pub fn with_entry(mut self, binding: &ResourceBinding) -> Self {
        self.add_entry(binding);
        self
    }

    /// Add an entry into the bind group layout
    pub fn add_entry(&mut self, binding: &ResourceBinding) {
        let slot = self.entries.len() as u32;

        self.entries.push(wgpu::BindGroupLayoutEntry {
            binding: slot,
            visibility: binding.visibility,
            ty: binding.ty,
            count: None,
        });
    }
}

impl GpuResource for LayoutBlueprint {
    type Id = LayoutId;

    fn create_id(&self) -> Self::Id {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        LayoutId(Uuid::from_u128(hasher.finish() as u128))
    }
}

/// Options for a storage texture in a `TextureBinding`
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct Storage {
    /// The access marker for the storage texture. Default is `ReadOnly`
    pub access: wgpu::StorageTextureAccess, 
    /// The format of the storage texture
    pub format: wgpu::TextureFormat,
    /// the view dimensions of the storage texture. Default is `D2`
    pub dims: wgpu::TextureViewDimension,
}

impl From<Storage> for wgpu::BindingType {
    fn from(value: Storage) -> Self {
        wgpu::BindingType::StorageTexture { 
            access: value.access, 
            format: value.format, 
            view_dimension: value.dims,
        }
    }
}

impl TextureBinding<Storage> {
    /// Create a `TextureBinding` for use as a storage texture.
    /// 
    /// Note that the `wgpu::Texture` mapped to `target` must be configured with the `STORAGE_TEXTURE` usage.
    pub fn storage(target: TextureId, format: wgpu::TextureFormat) -> Self {
        Self {
            target,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            binding_options: Storage { 
                format, 
                access: wgpu::StorageTextureAccess::ReadOnly, 
                dims: wgpu::TextureViewDimension::D2
            }
        }
    }

    /// Set the shader access for the storage texture to read only. 
    /// 
    /// Texture must be annotated with `read` (wgsl) or `read_only` (glsl) in the shader
    pub fn read_only(mut self) -> Self {
        self.binding_options.access = wgpu::StorageTextureAccess::ReadOnly;
        self
    }

    /// Set the shader access for the storage texture to read and write. 
    /// 
    /// Texture must be annotated with `read_write` in the shader.
    pub fn read_write(mut self) -> Self {
        self.binding_options.access = wgpu::StorageTextureAccess::ReadWrite;
        self
    }

    /// Set the shader access for the storage texture to atomic read write. 
    /// 
    /// Texture must be annotated with `read_write` in the shader.
    pub fn atomic_rw(mut self) -> Self {
        self.binding_options.access = wgpu::StorageTextureAccess::Atomic;
        self
    }

    /// Set the view dimensions of the storage texture.
    pub fn view_dimensions(mut self, dims: wgpu::TextureViewDimension) -> Self {
        self.binding_options.dims = dims;
        self
    }
}

/// Options for a sampled texture in a `TextureBinding`
pub struct Sampled {
    pub ty: wgpu::TextureSampleType,
    pub multisampled: bool,
    pub dims: wgpu::TextureViewDimension
}

impl From<Sampled> for wgpu::BindingType {
    fn from(value: Sampled) -> Self {
        wgpu::BindingType::Texture { 
            sample_type: value.ty, 
            view_dimension: value.dims, 
            multisampled: value.multisampled
        }
    }
}

impl Default for Sampled {
    fn default() -> Self {
        Self {
            ty: wgpu::TextureSampleType::Float { filterable: false },
            multisampled: false,
            dims: wgpu::TextureViewDimension::D2,
        }
    }
}

impl TextureBinding<Sampled> {
    /// Create a `TextureBinding` for use as a sampled texture.
    /// 
    /// Note that the `wgpu::Texture` mapped to `target` must be configured with the `TEXTURE_BINDING` usage (set by default).
    pub fn sampled(target: TextureId) -> Self {
        Self {
            target,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            binding_options: Sampled::default()
        }
    }

    /// Allow the texture to be filtered when sampled. Sampler returns an `f32`
    /// 
    /// The `wgpu::Sampler` used must be a filtering sampler (i.e. with the `Linear` filter mode).
    pub fn filterable(mut self) -> Self {
        self.binding_options.ty = wgpu::TextureSampleType::Float { filterable: true };
        self
    }

    /// Disallow the texture to be filtered when sampled. Sampler returns an `f32`.
    /// 
    /// The `wgpu::Sampler` used must be a non-filtering sampler (i.e. with the `Nearest` filter mode).
    pub fn non_filterable(mut self) -> Self {
        self.binding_options.ty = wgpu::TextureSampleType::Float { filterable: false };
        self
    }

    /// Sampler does depth comparisons. Paired sampler must be the `Comparison` type.
    pub fn depth(mut self) -> Self {
        self.binding_options.ty = wgpu::TextureSampleType::Depth;
        self
    }

    /// Sampling returns signed integers
    pub fn s_int(mut self) -> Self {
        self.binding_options.ty = wgpu::TextureSampleType::Sint;
        self
    }

    /// Sampler returns unsigned integers
    pub fn u_int(mut self) -> Self {
        self.binding_options.ty = wgpu::TextureSampleType::Uint;
        self
    }

    /// Allow the texture to be sampled multiple times.
    /// 
    /// The `wgpu::Texture` sampled must have a `sample_count` greater than 1 for this to take effect.
    /// 
    /// You can set the `sample_count` for the texture by calling `with_sample_count()` on the `TextureBlueprint` used to construct the `wgpu::Texture`.
    pub fn multisampled(mut self) -> Self {
        self.binding_options.multisampled = true;
        self
    }

    /// Set the view dimensions of the sampled texture. 
    pub fn view_dimensions(mut self, dims: wgpu::TextureViewDimension) -> Self {
        self.binding_options.dims = dims;
        self
    }
}

/// Represents a texture binding in a `wgpu::BindGroup`.
/// 
/// Default visibility is `VERTEX_FRAGMENT`
pub struct TextureBinding<T> {
    /// The target texture to be used in the binding.
    target: TextureId,
    /// Options for how the texture is used in the shader
    binding_options: T,
    /// The shader stages the texture is visible in. Default is `VERTEX_FRAGMENT`.
    visibility: wgpu::ShaderStages,
}

impl<T> TextureBinding<T> {
    /// Sets the texture binding to be visible only in a vertex shader.
    pub fn vertex_visible(mut self) -> Self {
        self.visibility = wgpu::ShaderStages::VERTEX;
        self
    }
    
    /// Sets the texture binding to be visible only in a fragment shader.
    pub fn fragment_visible(mut self) -> Self {
        self.visibility = wgpu::ShaderStages::FRAGMENT;
        self
    }

    /// Sets the texture binding to be visible in a compute shader.
    pub fn compute_visible(mut self) -> Self {
        self.visibility |= wgpu::ShaderStages::COMPUTE;
        self
    }
}

impl<T: Into<wgpu::BindingType>> Bindable for TextureBinding<T> {
    fn into_binding(self) -> ResourceBinding {
        ResourceBinding {
            target: BindingTarget::Texture(self.target),
            ty: self.binding_options.into(),
            visibility: self.visibility
        }
    }
}

/// Represents a buffer binding and entry in a bind group
pub struct BufferBinding {
    /// The target buffer to be used in the binding
    target: BufferId,
    /// The buffer binding type
    ty: wgpu::BufferBindingType,
    /// The shader stages the buffer is visible in. Default is `VERTEX_FRAGMENT`
    visibility: wgpu::ShaderStages,
    /// Determines if the buffer has a dynamic offset when used in a `DrawCommand`
    has_dyn_offset: bool,
    /// Determines if the buffer binding has a minimum size and what that size is.
    min_binding_size: Option<NonZero<u64>>
}

impl BufferBinding {
    /// Create a new buffer binding with the provided binding type.
    pub fn new(target: BufferId, ty: wgpu::BufferBindingType) -> Self {
        Self {
            target,
            ty,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            has_dyn_offset: false,
            min_binding_size: None,
        }
    }

    /// Create a new storage buffer binding
    pub fn storage(target: BufferId, read_only: bool) -> Self {
        let ty = wgpu::BufferBindingType::Storage { read_only };
        BufferBinding::new(target, ty)
    }

    /// Create a new uniform buffer binding
    pub fn uniform(target: BufferId) -> Self {
        let ty = wgpu::BufferBindingType::Uniform;
        BufferBinding::new(target, ty)
    }

    /// Set the shader stage visibility for the buffer binding
    pub fn with_visibility(mut self, visibility: wgpu::ShaderStages) -> Self {
        self.visibility = visibility;
        self
    }

    /// Set the binding to have a dynamic offset
    pub fn with_dynamic_offset(mut self) -> Self {
        self.has_dyn_offset = true;
        self
    }

    /// Set the minimum buffer size for the binding.
    pub fn with_min_size(mut self, size: NonZero<u64>) -> Self {
        self.min_binding_size = Some(size);
        self
    }
}

impl Bindable for BufferBinding {
    fn into_binding(self) -> ResourceBinding {
        ResourceBinding {
            target: BindingTarget::Buffer(self.target),
            ty: wgpu::BindingType::Buffer { 
                ty: self.ty, 
                has_dynamic_offset: self.has_dyn_offset, 
                min_binding_size: self.min_binding_size 
            },
            visibility: self.visibility
        }
    }
}

/// Represents the binding of a Sampler in a bind group
pub struct SamplerBinding {
    /// The target id for the sampler used in the sampler binding
    target: SamplerId,
    /// The shader stages the sampler is visible in.
    visibility: wgpu::ShaderStages,
    /// The binding type of the sampler
    ty: wgpu::SamplerBindingType,
}

impl SamplerBinding {
    /// Sampler result is produced from more than one color sample.
    pub fn filtering(target: SamplerId) -> Self {
        Self {
            target,
            ty: wgpu::SamplerBindingType::Filtering,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        }
    }

    /// Sampler result is produced from just one color sample.
    pub fn non_filtering(target: SamplerId) -> Self {
        Self {
            target,
            ty: wgpu::SamplerBindingType::NonFiltering,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        }
    }

    /// Sampler result is produced from comparing color samples, based on the sampler compare function.
    pub fn comparison(target: SamplerId) -> Self {
        Self {
            target,
            ty: wgpu::SamplerBindingType::Comparison,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        }
    }

    /// Set the shader stage visibility for the sampler binding
    pub fn with_visibility(mut self, visibility: wgpu::ShaderStages) -> Self {
        self.visibility = visibility;
        self
    }
}

impl Bindable for SamplerBinding {
    fn into_binding(self) -> ResourceBinding {
        ResourceBinding {
            target: BindingTarget::Sampler(self.target),
            ty: wgpu::BindingType::Sampler(self.ty),
            visibility: self.visibility
        }
    }
}