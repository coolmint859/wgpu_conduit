use uuid::Uuid;

/// A unique identifier for a `wgpu::Buffer`
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct BufferId(pub Uuid);

impl BufferId {
    /// An uninitialized `BufferId` (refers to nothing). 
    pub const UNINIT: Self = BufferId(Uuid::nil());
}

/// A unique identifier for a `wgpu::Texture`
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub Uuid);

impl TextureId {
    /// An uninitialized `TextureId` (refers to nothing). 
    pub const UNINIT: Self = TextureId(Uuid::nil());
}

/// A content-addressable identifier for a `wgpu::Sampler`
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct SamplerId(pub Uuid);

impl SamplerId {
    /// An uninitialized `SamplerId` (refers to nothing). 
    pub const UNINIT: Self = SamplerId(Uuid::nil());
}

/// A unique identifier for a `wgpu::BindGroup`
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct BindGroupId(pub Uuid);

impl BindGroupId {
    /// An uninitialized `BindGroupId` (refers to nothing). 
    pub const UNINIT: Self = BindGroupId(Uuid::nil());
}

/// A content-addressable identifier for a `wgpu::BindGroupLayout`
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct LayoutId(pub Uuid);

impl LayoutId {
    /// An uninitialized `LayoutId` (refers to nothing). 
    pub const UNINIT: Self = LayoutId(Uuid::nil());
}

/// Unique identifiers for a `wgpu::BindGroup` and associated `wgpu::BindGroupLayout`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct BindGroupIdPair {
    pub id: BindGroupId,
    pub layout_id: LayoutId,
}

impl BindGroupIdPair {
    /// An uninitialized `BindGroupIdPair` (ids refer to nothing). 
    pub const UNINIT: Self = BindGroupIdPair {
        id: BindGroupId::UNINIT,
        layout_id: LayoutId::UNINIT
    };
}

/// A unique identifier for a `wgpu::ShaderModule`
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct ShaderId(pub Uuid);

impl ShaderId {
    /// An uninitialized `ShaderId` (refers to nothing). 
    pub const UNINIT: Self = ShaderId(Uuid::nil());
}

/// A content-addressable identifier for a `wgpu::RenderPipeline` or `wgpu::ComputePipeline`.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct PipelineId(pub Uuid);

impl PipelineId {
    /// An uninitialized `PipelineId` (refers to nothing). 
    pub const UNINIT: Self = PipelineId(Uuid::nil());
}