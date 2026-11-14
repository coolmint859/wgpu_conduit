use std::{ops::Deref};

use async_store::HoldPolicy;
use image::GenericImageView;
use uuid::Uuid;

use crate::context::*;

/// A lightweight handle to a gpu texture
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TextureHandle {
    /// The handle to the actual wgpu `Texture`
    pub texture: wgpu::Texture,
    /// The view into the associated wgpu `Texture`
    pub view: wgpu::TextureView,
    /// The dimensions (width/height/depth) of the texture
    pub extent: wgpu::Extent3d,
}

impl Deref for TextureHandle {
    type Target = wgpu::TextureView;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

/// Specifies the dimensions of a texture
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TextureDimensions {
    pub width: u32,
    pub height: u32, 
    pub depth: u32,
    pub wgpu_dim: wgpu::TextureDimension
}

impl TextureDimensions {
    /// A 2D texture
    pub fn size_2d(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            depth: 1,
            wgpu_dim: wgpu::TextureDimension::D2
        }
    }

    /// A 3D texture
    pub fn size_3d(width: u32, height: u32, depth: u32) -> Self {
        Self {
            width,
            height,
            depth,
            wgpu_dim: wgpu::TextureDimension::D3
        }
    }

    /// Returns `self` as a `wgpu::Extent3d`
    pub fn as_extent(&self) -> wgpu::Extent3d {
        wgpu::Extent3d {
            width: self.width,
            height: self.height,
            depth_or_array_layers: self.depth,
        }
    }
}

/// Represents sources for textures, condensing the blueprint into a payload for use during uploading.
pub trait TextureSource: Send + 'static {
    /// Converts `self` into the raw texture data and dimensions given a `format`, if possible.
    fn into_info(self) -> Result<TextureSourceInfo, String>;
}

/// Texture source information for creating `wgpu::Textures`
pub struct TextureSourceInfo {
    /// Initial texture data. Must match exactly the dimensions and format of the accompanying blueprint.
    /// If `None`, the texture will be initialized with the default values (usually 0s) according the blueprint format.
    pub init_data: Option<Vec<u8>>,
    /// The dimensions of the texture. 
    /// 
    /// If `init_data` is `Some(Vec<u8>)`, the dimensions and format must combine to be exactly equal to the length of the byte data.
    pub dim: TextureDimensions,
    /// The format of the texture. 
    /// 
    /// If `init_data` is `Some(Vec<u8>)`, the dimensions and format must combine to be exactly equal to the length of the byte data.
    pub format: wgpu::TextureFormat,
}

// allows source info struct to be provided directly.
impl TextureSource for TextureSourceInfo {
    fn into_info(self) -> Result<TextureSourceInfo, String> {
        Ok(self)
    }
}

/// A `TextureSource` for textures created from image data
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct OnDisk {
    /// The path to the image data
    pub path: &'static str,
    /// The format of the texture.
    pub format: wgpu::TextureFormat,
}

impl TextureSource for OnDisk {
    fn into_info(self) -> Result<TextureSourceInfo, String> {
        match image::open(self.path)
            .map_err(|err| format!("Failed to read image file with path {}: {}", self.path, err)) 
        {
            Ok(img) => {
                let dim = TextureDimensions {
                    width: img.width(),
                    height: img.height(),
                    depth: 1,
                    wgpu_dim: wgpu::TextureDimension::D2
                };

                let raw_data = match self.format {
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
                        img.to_bgra8().into_raw()
                    },
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
                        img.to_rgba8().into_raw()
                    },
                    wgpu::TextureFormat::R8Unorm => {
                        img.to_luma8().into_raw()
                    }
                    _ => {
                        return Err(format!(
                            "Unsupported Texture Format {:?} for image data from path @'{}'", self.format, self.path
                        ));
                    }
                };

                Ok(TextureSourceInfo {
                    init_data: Some(raw_data),
                    dim,
                    format: self.format
                })
            },
            Err(e) => return Err(e)
        }
    }
}

/// A `TextureSource` for textures created from procedural generation
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Procedural { 
    /// The procedurally generated texture data. Must match the dimensions
    pub data: Vec<u8>,
    /// The dimensions (width/height/depth) of the texture data
    pub dim: TextureDimensions,
    /// The format of the texture. 
    /// 
    /// The dimensions and format must combine to be exactly equal to the length of the byte data.
    pub format: wgpu::TextureFormat,
}

impl TextureSource for Procedural {
    fn into_info(self) -> Result<TextureSourceInfo, String> {
        Ok(TextureSourceInfo { 
            init_data: Some(self.data), 
            dim: self.dim, 
            format: self.format
        })
    }
}

/// A `TextureSource` for textures created by running a compute shader
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Computed {
    /// The dimensions (width/height/depth) of the output from the compute shader
    pub dim: TextureDimensions,
    /// The format of the texture.
    pub format: wgpu::TextureFormat,
}

impl TextureSource for Computed {
    fn into_info(self) -> Result<TextureSourceInfo, String> {
        Ok(TextureSourceInfo { 
            init_data: None, 
            dim: self.dim, 
            format: self.format
        })
    }
}

/// A blueprint for constructing wgpu textures.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TextureBlueprint {
    /// Label for gpu profiling
    pub label: String,
    /// The usage of the texture, default is `wgpu::TextureUsages::TEXTURE_BINDING`
    pub usage: wgpu::TextureUsages,
    /// The mip levels of the texture, default is 1.
    pub mip_levels: u32,
    /// The hold policy of the texture. If `None`, the global policy is used instead
    pub hold_policy: Option<HoldPolicy>
}

impl TextureBlueprint {
    pub fn new() -> Self {
        Self {
            label: "Texture".to_string(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            mip_levels: 1,
            hold_policy: None,
        }
    }

    /// Create a texture blueprint configured as a render attachment
    pub fn render_attachment() -> Self {
        Self {
            label: "depth texture".to_string(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            mip_levels: 1,
            hold_policy: None
        }
    }

    /// Set the label for gpu profiling of the resultant texture
    pub fn with_label(mut self, label: &str) -> Self {
        self.label = label.to_string();
        self
    }

    /// Set the hold policy for this sampler. This overrides the global policy.
    pub fn with_hold_policy(mut self, policy: HoldPolicy) -> Self {
        self.hold_policy = Some(policy);
        self
    }

    /// Set the mipmap levels for the texture.
    pub fn with_mip_levels(mut self, levels: u32) -> Self {
        self.mip_levels = levels;
        self
    }

    /// Add a additional usage for the texture
    pub fn with_additional_usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.usage |= usage;
        self
    }

    /// Allow the texture to be written to
    pub fn writable(mut self) -> Self {
        self.usage |= wgpu::TextureUsages::COPY_DST;
        self
    }

    /// Allow the texture to be read from
    pub fn readable(mut self) -> Self {
        self.usage |= wgpu::TextureUsages::COPY_SRC;
        self
    }
    
    /// Allow the texture to be written to and read from
    pub fn read_write(mut self) -> Self {
        self.usage |= wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC;
        self
    }

    /// Allows the texture to be used for storage in a bind group
    pub fn storage_bindable(mut self) -> Self {
        self.usage |= wgpu::TextureUsages::STORAGE_BINDING;
        self
    }

    /// Allows the texture to be used in a bind group as a standard texture.
    /// 
    /// When created with `new()`, this usage is configured by default.
    pub fn standard_bindable(mut self) -> Self {
        self.usage |= wgpu::TextureUsages::TEXTURE_BINDING;
        self
    }
}

impl GpuResource for TextureBlueprint {
    type Id = TextureId;

    fn create_id(&self) -> Self::Id {
        TextureId(Uuid::new_v4())
    }
}

pub mod texture_utils {
    /// Determine the number of bytes per pixel based on the currently set format
    pub fn bytes_per_pixel(format: wgpu::TextureFormat) -> u32 {
        match format {
            wgpu::TextureFormat::R8Unorm | wgpu::TextureFormat::R8Snorm => 1,
            wgpu::TextureFormat::Rg8Unorm | wgpu::TextureFormat::Rg8Snorm => 2,
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Bgra8UnormSrgb => 4,
            wgpu::TextureFormat::R32Float => 4,
            wgpu::TextureFormat::Rgba16Float => 8,
            wgpu::TextureFormat::Rgba32Float => 16,
            _ => panic!("Unsupported texture format for automatic layout calculation: {:?}", format),
        }
    }
}