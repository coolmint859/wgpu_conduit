use crate::{Computed, GpuContext, TexDimensions, Texture, TextureHandle};

/// Represents color and depth attachments onto a `Canvas`, when added to a `GraphicsInit` struct.
pub trait RenderAttachment {
    /// The name for the attachment. 
    /// 
    /// This is used as the key to query the Canvas for attachments during runtime to use in `Pipeline`s and `DrawCommand`s.
    fn name(&self) -> &str;

    /// Converts `self` into a static `AttachmentConfig`.
    fn into_config(self) -> AttachmentConfig;
}

/// A standard depth attachment with format `Depth32Float`
/// 
/// Best used for depth testing in 2d and 3d scenes.
pub struct StandardDepth;
impl RenderAttachment for StandardDepth {
    fn name(&self) -> &str { "depth_atm" }

    fn into_config(self) -> AttachmentConfig {
        AttachmentConfig {
            blueprint: Texture::render_attachment(wgpu::TextureFormat::Depth32Float).with_label(self.name()),
            dims: AttachmentDimensions::Canvas,
        }
    }
}

/// Used to to specify how to create the attachment texture with the `Canvas` and also how to resize it when the `Canvas` resizes.
/// Textures stored on a `Canvas` are *persistent* - they live for as long as the `Canvas` lives.
/// 
/// Attachment textures are created *synchronously*, as their source data is assumed to be computed on the gpu.
/// This is acceptable in most cases since most rendering architectures do this.
/// 
/// In the case the texture data is *not* created on the gpu, it's recommended to use a storage texture instead as that is better supported in that case than with render attachments.
#[derive(Debug, Clone)]
pub struct AttachmentConfig {
    /// The attachment texture blueprint. This is used in the creation of the underlying texture that the attachment represents.
    /// 
    /// All attachments created needs to have at least the `RENDER_ATTACHMENT` usage flag set. `Texture::render_attachment()` can be called to set this usage flag for you.
    pub blueprint: Texture,
    /// The dimensions of the attachment. This is used to determine how large the attachment should be when created, as well as how it should be resized when the canvas is resized.
    pub dims: AttachmentDimensions,
}

impl AttachmentConfig {
    /// Creates a new texture handle from this config's blueprint with the provided dimensions.
    pub(crate) fn create_handle(&self, width: u32, height: u32, context: &mut GpuContext) -> TextureHandle {
        let dim = self.dims.as_tex_dims(width, height);
        
        context
            .gpu_handle()
            .create_texture(self.blueprint.clone(), Computed { dim })
            .expect(&format!("Failed to create texture handle for render attachment with label {}.", self.blueprint.label)) 
    }
}

/// Defines how a render attachment should be resized when the canvas resizes.
#[derive(Debug, Clone, Copy)]
pub enum AttachmentDimensions {
    /// Attachment dimensions are the same as the canvas.
    /// 
    /// If the canvas is resized, the attachment is resized to match.
    Canvas,
    /// Attachment dimensions are a fixed size regardless of whether the canvas resizes.
    /// 
    /// Note: Consider using a `Sampler` during rendering to prevent artifacts.
    Fixed { width: u32, height: u32 },
    /// Attachment dimensions are a scaled multiple of the canvas dimensions (i.e. A value of `0.5` means half the size of the canvas).
    /// 
    /// Note: Consider using a `Sampler` during rendering to prevent artifacts
    Scaled(f32)
}

impl AttachmentDimensions {
    /// Converts `self` into texture dimensions as used when requesting a texture through the `GpuContext`. Does not consume `self`
    pub(crate) fn as_tex_dims(&self, canvas_width: u32, canvas_height: u32) -> TexDimensions {
        match self {
            AttachmentDimensions::Canvas => { 
                TexDimensions::size_2d(canvas_width, canvas_height) 
            }
            AttachmentDimensions::Fixed { width, height } => {
                TexDimensions::size_2d(*width, *height)
            }
            AttachmentDimensions::Scaled(scale) => {
                let width = (canvas_width as f32 / scale) as u32;
                let height = (canvas_height as f32 / scale) as u32;

                TexDimensions::size_2d(width, height)
            }
        }
    }
}

/// Render attachment texture and config, used for pipelines and during render passes.
pub(crate) struct AttachmentRecord {
    /// The handle to the wgpu texture, view, and extent
    pub(crate) tex: TextureHandle,
    /// configuration of how the recreate the texture during resizing
    pub(crate) config: AttachmentConfig,
}

impl AttachmentRecord {
    /// Resizes the attachment texture if the dimensions are not fixed.
    pub(crate) fn resize(&mut self, canvas_width: u32, canvas_height: u32, context: &mut GpuContext) {
        match self.config.dims {
            AttachmentDimensions::Canvas => {
                self.tex = self.config.create_handle(canvas_width, canvas_height, context);
            },
            AttachmentDimensions::Scaled(scale) => {
                let width = (canvas_width as f32 / scale) as u32;
                let height = (canvas_height as f32 / scale) as u32;

                self.tex = self.config.create_handle(width, height, context)
            },
            // fixed attachments are never resized
            AttachmentDimensions::Fixed {..} => {}
        }
    }
}