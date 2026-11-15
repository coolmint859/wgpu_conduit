use uuid::Uuid;

/// Unique identifier for a buffer
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct BufferId(pub Uuid);

impl BufferId {
    /// An uninitialized `BufferId` (refers to nothing). 
    pub const UNINIT: Self = BufferId(Uuid::nil());
}

/// Unique identifier for a texture
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureId(pub Uuid);

impl TextureId {
    /// An uninitialized `TextureId` (refers to nothing). 
    pub const UNINIT: Self = TextureId(Uuid::nil());
}

/// Content-Addressable identifier for a sampler
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct SamplerId(pub Uuid);

impl SamplerId {
    /// An uninitialized `SamplerId` (refers to nothing). 
    pub const UNINIT: Self = SamplerId(Uuid::nil());
}

/// Unique identifier for a bind group
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct BindGroupId(pub Uuid);

impl BindGroupId {
    /// An uninitialized `BindGroupId` (refers to nothing). 
    pub const UNINIT: Self = BindGroupId(Uuid::nil());
}

/// Content-Addressable identifier for a bind group layout
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct LayoutId(pub Uuid);

impl LayoutId {
    /// An uninitialized `LayoutId` (refers to nothing). 
    pub const UNINIT: Self = LayoutId(Uuid::nil());
}

/// Unique identifiers for a bind group and associated layout
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

/// Unique identifier for a shader module
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct ShaderId(pub Uuid);

impl ShaderId {
    /// An uninitialized `ShaderId` (refers to nothing). 
    pub const UNINIT: Self = ShaderId(Uuid::nil());
}

/// Content-Addressable identifier for a pipeline
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] 
pub struct PipelineId(pub Uuid);

impl PipelineId {
    /// An uninitialized `PipelineId` (refers to nothing). 
    pub const UNINIT: Self = PipelineId(Uuid::nil());
}