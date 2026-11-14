use std::collections::HashMap;

use crate::{GpuContext, canvas::{CanvasDimensions, RenderAttachment, RenderAttachmentRecord}};

/// The current surface texture on the swapchain, as well as it's view.
pub struct SurfaceHandle {
    pub(crate) texture: wgpu::SurfaceTexture,
    pub(crate) view: wgpu::TextureView
}

/// Contains the set of render attachments to be used during rendering.
#[derive(Debug)]
pub struct FrameBuffer {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) attachments: HashMap<String, RenderAttachmentRecord>,
}

impl FrameBuffer {
    /// resize the canvas surface, color, and depth target textures
    pub(crate) fn resize(&mut self, width: u32, height: u32, context: &GpuContext) {
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&context.gpu.device, &self.config);

        for atm in self.attachments.values_mut() {
            atm.resize(width, height, context);
        }
    }

    /// Returns a handle to the next surface texture in the swapchain.
    pub fn swap_surface(&self) -> Result<SurfaceHandle, wgpu::SurfaceError> {
        let surface_tex = self.surface.get_current_texture()?;
        let view = surface_tex.texture.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(SurfaceHandle {
            texture: surface_tex,
            view
        })
    }

    /// Returns the width of the surface texture.
    pub fn surface_width(&self) -> u32 {
        self.config.width
    }

    /// Returns the height of the surface texture.
    pub fn surface_height(&self) -> u32 {
        self.config.height
    }

    /// Returns the dimensions of the surface texture.
    pub fn surface_dimensions(&self) -> CanvasDimensions {
        CanvasDimensions { 
            width: self.config.width, 
            height: self.config.height 
        }
    }

    /// Returns the aspect ratio of the surface texture. 
    pub fn surface_aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    /// Returns the format of the surface texture
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    // /// Returns the format of an attachment texture if mapped to the provided `RenderAttachment` implementation.
    // pub fn attachment_format(&self, atm: impl RenderAttachment) -> Option<wgpu::TextureFormat> {
    //     let name = atm.name();
    //     let record = self.attachments.get(name)?;

    //     Some(record.config.format)
    // }

    /// Returns a `wgpu::ColorTargetState` for a given render attachment with the provided blend and write options, if exists.
    pub fn color_state(
        &self, 
        atm: impl RenderAttachment, 
        blend: Option<wgpu::BlendState>, 
        write: wgpu::ColorWrites
    ) -> Option<wgpu::ColorTargetState> 
    {
        let name = atm.name();
        let record = self.attachments.get(name)?;

        Some(wgpu::ColorTargetState {
            format: record.config.blueprint.format,
            blend,
            write_mask: write
        })
    }

    /// Returns a `wgpu::DepthStencilState` with the provided compare fn and write enabled options, if exists.
    pub fn depth_state(
        &self,
        atm: impl RenderAttachment,
        compare: wgpu::CompareFunction,
        write_enabled: bool,
    ) -> Option<wgpu::DepthStencilState> {
        let name = atm.name();
        let record = self.attachments.get(name)?;

        Some(wgpu::DepthStencilState {
            format: record.config.blueprint.format,
            depth_compare: compare,
            depth_write_enabled: write_enabled,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default() 
        })
    }
}