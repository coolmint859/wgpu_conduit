use async_store::HoldPolicy;
use uuid::Uuid;

use crate::context::*;

/// Represents types that can be serialized into slices of bytes.
/// 
/// All types that implement `bytemuck::Pod` and `bytemuck::Zeroable` 
/// automatically implement `Serializable`.
pub trait Serializable {
    /// Converts `self` into a slice of bytes
    fn to_bytes(&self) -> &[u8];
}

impl<T> Serializable for T 
where T: bytemuck::Pod + bytemuck::Zeroable
{
    fn to_bytes(&self) -> &[u8] { bytemuck::bytes_of(self) }
}

/// A wrapper around initial data sources for gpu buffers
pub struct BufferSource {
    pub data: Option<Vec<u8>>,
    pub capacity: u64,
}

impl BufferSource {
    /// No initial data.
    /// 
    /// If the accompanying `BufferBlueprint` is specified with the uniform usage, then `capacity` is rounded up to the nearest multiple of 16.
    pub fn empty(capacity: u64) -> Self {
        Self {
            data: None,
            capacity,
        }
    }

    /// Initial data from a type `T` that implements `Serializable`,
    ///
    /// All types that implement `bytemuck::Pod` and `bytemuck::Zeroable` automatically implement `Serializable`
    pub fn from_pod<T: Serializable>(data: T) -> Self {
        let bytes = data.to_bytes().to_vec();

        Self {
            capacity: bytes.len() as u64,
            data: Some(bytes),
        }
    }

    /// Initial data from a raw byte vector
    pub fn from_raw(data: Vec<u8>) -> Self {
        Self {
            capacity: data.len() as u64,
            data: Some(data),
        }
    }

    /// Initial data from a vector of types that implement `Serializable` and `bytemuck::NoUninit`
    /// 
    /// All types that implement `bytemuck::Pod` and `bytemuck::Zeroable` automatically implement `Serializable`
    pub fn from_vec<T: Serializable + bytemuck::NoUninit>(data: Vec<T>) -> Self {
        let bytes = bytemuck::cast_slice(&data).to_vec();

        Self {
            capacity: bytes.len() as u64,
            data: Some(bytes),
        }
    }

    /// Set the capacity of the buffer populated with this source data.
    /// 
    /// If `capacity` is less then the provided data's byte length, then the length of the data is used instead and `capacity` is ignored.
    /// 
    /// If the buffer this source is populated with is a uniform buffer, then `capacity` 
    /// is rounded to the nearest 16 bytes, in accordance with wgpu uniform buffer alignment rules.
    pub fn with_capacity(mut self, capacity: u64) -> Self {
        let capacity = match &self.data {
            Some(data)  => (data.len() as u64).max(capacity),
            None => capacity
        };

        self.capacity = capacity;
        self
    }
}


/// Represents an update into an existing buffer
pub trait BufferUpdate {
    /// The data payload as an array slice of bytes
    fn bytes(&self) -> &[u8];
    /// The offset at which to insert the data at into the buffer.
    fn offset(&self) -> u64;
}

/// A buffer update from any type that implements the `Serializable` trait
/// 
/// Types which implement `bytemuck::POD` and `bytemuck::Zeroable` automatically implement `Serializable`
pub struct StructuredUpdate<'a, T: Serializable> {
    /// The struct payload
    pub data: &'a T,
    /// The offset at which to insert the struct data at into the buffer
    pub offset: u64,
}

impl<'a, T: Serializable> BufferUpdate for StructuredUpdate<'a, T> {
    #[inline]
    fn bytes(&self) -> &[u8] { self.data.to_bytes() }

    #[inline]
    fn offset(&self) -> u64 { self.offset }
}

/// A buffer update from raw bytes, inserted at an offset
pub struct RawBytesUpdate<'a> {
    /// The raw data payload
    pub data: &'a [u8],
    /// The offset at which to insert the raw data at into the buffer
    pub offset: u64,
}

impl<'a> BufferUpdate for RawBytesUpdate<'a> {
    #[inline]
    fn bytes(&self) -> &[u8] { self.data }

    #[inline]
    fn offset(&self) -> u64 { self.offset }
}

/// A blueprint for constructing wgpu Buffers, where T is the buffer type.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BufferBlueprint {
    pub label: String,
    pub usage: wgpu::BufferUsages,
    pub alignment: Option<u64>,
    pub hold_policy: Option<HoldPolicy>
}

impl BufferBlueprint {
    /// Create a buffer blueprint with the uniform usage type.
    /// 
    /// All Uniform buffers have a default capacity of 16 to follow wgpu alignment rules
    pub fn uniform() -> Self {
        Self {
            label: "uniform_buffer".to_string(),
            usage: wgpu::BufferUsages::UNIFORM,
            alignment: Some(16),
            hold_policy: None,
        }
    }

    /// Create a buffer blueprint with the storage usage type.
    pub fn storage() -> Self {
        Self {
            label: "storage_buffer".to_string(),
            usage: wgpu::BufferUsages::STORAGE,
            alignment: None,
            hold_policy: None,
        }
    }

    /// Create a buffer blueprint with the vertex usage type.
    pub fn vertex() -> Self {
        Self {
            label: "vertex_buffer".to_string(),
            usage: wgpu::BufferUsages::VERTEX,
            alignment: None,
            hold_policy: None,
        }
    }

    /// Create a buffer blueprint with the index usage type.
    pub fn index() -> Self {
        Self {
            label: "index_buffer".to_string(),
            usage: wgpu::BufferUsages::INDEX,
            alignment: None,
            hold_policy: None,
        }
    }

    /// Set the label for gpu profiling of the resultant buffer
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    /// Set the hold policy for this buffer. This overrides the global policy.
    pub fn with_hold_policy(mut self, policy: HoldPolicy) -> Self {
        self.hold_policy = Some(policy);
        self
    }

    /// Add an additional usage flag to the resultant buffer
    pub fn with_additional_usage(mut self, usage: wgpu::BufferUsages) -> Self {
        self.usage |= usage;
        self
    }

    /// Allow the buffer to be written to.
    pub fn writable(mut self) -> Self {
        self.usage |= wgpu::BufferUsages::COPY_DST;
        self
    }

    /// Allow the buffer to be read from.
    pub fn readable(mut self) -> Self {
        self.usage |= wgpu::BufferUsages::COPY_SRC;
        self
    }

    /// Allow the buffer to be read from and writable to.
    pub fn read_write(mut self) -> Self {
        self.usage |= wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
        self
    }

    /// Pads `data` with 0s to align the length to a multiple of `alignment`
    pub fn pad_bytes(mut data: Vec<u8>, alignment: usize) -> Vec<u8> {
        let remainder = data.len() % alignment;
        if remainder > 0 {
            let padding = alignment - remainder;
            data.resize(data.len() + padding, 0u8);
        }

        data
    }

    /// Rounds `value` to the nearest multiple of `alignment`.
    pub fn align(mut value: u64, alignment: u64) -> u64 {
        let remainder = value % alignment;
        if remainder > 0 {
            let padding = alignment - remainder;
            value += padding;
        }

        value
    }

    /// Extends a byte vector `data` with 0s up to `capacity`.
    /// 
    /// If the length of `data` is already larger than `capacity`, this method does nothing.
    pub fn extend_bytes(mut data: Vec<u8>, capacity: usize) -> Vec<u8> {
        if capacity - data.len() > 0 {
            data.resize(capacity, 0u8);
        }

        return data;
    }
}

impl GpuResource for BufferBlueprint {
    type Id = BufferId;

    fn create_id(&self) -> Self::Id {
        BufferId(Uuid::new_v4())
    }
}

/// Utility functions for buffer data
pub mod buffer_utils {
    use crate::Serializable;

    /// Pads `data` with 0s to align the length to a multiple of `alignment`
    pub fn pad_bytes(mut data: Vec<u8>, alignment: usize) -> Vec<u8> {
        let remainder = data.len() % alignment;
        if remainder > 0 {
            let padding = alignment - remainder;
            data.resize(data.len() + padding, 0u8);
        }

        data
    }

    /// Rounds `value` to the nearest multiple of `alignment`.
    pub fn align(mut value: u64, alignment: u64) -> u64 {
        let remainder = value % alignment;
        if remainder > 0 {
            let padding = alignment - remainder;
            value += padding;
        }

        value
    }

    /// Extends a byte vector `data` with 0s up to `capacity`.
    /// 
    /// If the length of `data` is already larger than `capacity`, this method does nothing.
    pub fn extend_bytes(mut data: Vec<u8>, capacity: usize) -> Vec<u8> {
        if capacity - data.len() > 0 {
            data.resize(capacity, 0u8);
        }

        return data;
    }

    /// Serializes a vector of `T` where T implements `Serializable` and `bytemuck::NoUninit`
    /// 
    /// This is functionally equivelant to `bytemuck::cast_vec()`
    pub fn serialize<T: Serializable + bytemuck::NoUninit>(data: Vec<T>) -> Vec<u8> {
        bytemuck::cast_vec(data)
    }
}