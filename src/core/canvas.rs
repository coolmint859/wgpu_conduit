use std::{collections::HashMap, sync::Arc};
use winit::window::{CursorGrabMode, Window};

use crate::{AttachmentRecord, ColorAttachment, DepthAttachment, DepthLoadOp, RenderAttachment, gpu_state::GpuContext};

/// The width and height of the `Canvas`
pub struct CanvasDimensions {
    pub width: u32,
    pub height: u32
}

/// Contains the rendering targets used by the canvas. 
pub struct CanvasTextures { 
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,

    pub(crate) colors: HashMap<String, AttachmentRecord>,
    pub(crate) depths: HashMap<String, AttachmentRecord>
}

impl CanvasTextures {
    /// resize the canvas surface, color, and depth textures
    pub(crate) fn resize(&mut self, width: u32, height: u32, context: &mut GpuContext) {
        self.config.width = width;
        self.config.height = height;

        self.surface.configure(&context.gpu.device, &self.config);

        for atm in &mut self.colors.values_mut() {
            atm.resize(width, height, context);
        }

        for atm in &mut self.depths.values_mut() {
            atm.resize(width, height, context);
        }
    }

    /// Returns the next surface texture if available.
    pub fn next_surface_texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    /// returns the current dimensions of the canvas surface texture.
    pub fn surface_dimensions(&self) -> CanvasDimensions {
        CanvasDimensions { width: self.config.width, height: self.config.height }
    }

    pub fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height as f32
    }

    /// Returns a `wgpu::ColorTargetState` for a given render attachment with the provided blend and write options, if exists.
    pub fn color_state(
        &self, 
        atm: impl RenderAttachment, 
        blend: Option<wgpu::BlendState>, 
        write: wgpu::ColorWrites
    ) -> Option<wgpu::ColorTargetState> 
    {
        let name = atm.name();
        let record = self.colors.get(name)?;

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
        let record = self.depths.get(name)?;

        Some(wgpu::DepthStencilState {
            format: record.config.blueprint.format,
            depth_compare: compare,
            depth_write_enabled: write_enabled,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default() 
        })
    }
}

/// The current rendering frame.
pub struct CanvasFrame<'a> {
    /// The rendering surface
    surface: wgpu::SurfaceTexture,
    /// known color attachments
    colors: &'a HashMap<String, AttachmentRecord>,
    /// depth attachmment
    depths: &'a HashMap<String, AttachmentRecord>,
    /// The current wgpu texture view onto the `Canvas`
    view: wgpu::TextureView,
    /// The current frame number
    pub number: u32,
    /// the width/height of the `Canvas`
    pub dimensions: CanvasDimensions
}

impl<'a> CanvasFrame<'a> {
    /// Returns the width of the frame.
    pub fn width(&self) -> u32{
        self.dimensions.width
    }

    /// Returns the width of the frame.
    pub fn height(&self) -> u32 {
        self.dimensions.height
    }

    /// Returns the aspect ratio of the frame.
    pub fn aspect(&self) -> f32 {
        self.dimensions.width as f32 / self.dimensions.height as f32
    }

    /// Present the frame to the surface texture. This consumes the frame.
    pub fn present(self) {
        self.surface.present();
    }

    /// Returns the current surface attachment for this frame.
    pub fn surface_attachment(&self, clear_color: Option<wgpu::Color>) -> ColorAttachment {
        ColorAttachment {
            view: self.view.clone(),
            clear_color
        }
    }

    /// Returns a color attachment as defined by the provided `RenderAttachment` implementation, if available.
    pub fn color_attachment(&self, atm: impl RenderAttachment, clear_color: Option<wgpu::Color>) -> Option<ColorAttachment> {
        let record = self.colors.get(atm.name())?;

        Some(ColorAttachment {
            view: record.tex.view.clone(),
            clear_color
        })
    }

    /// Returns a depth attachment as defined by the provided `RenderAttachment` implementation, if available.
    pub fn depth_attachment(&self, atm: impl RenderAttachment, load_op: DepthLoadOp) -> Option<DepthAttachment> {
        let record = self.depths.get(atm.name())?;

        Some(DepthAttachment {
            view: record.tex.view.clone(),
            op: load_op
        })
    }
}

/// Represents the window and rendering surface
pub struct Canvas {
    pub(crate) window: Arc<Window>,
    pub textures: CanvasTextures,

    is_cursor_locked: bool,
    is_focused: bool,
    frame_number: u32,
}

impl Canvas {
    pub(crate) fn new(window: Arc<Window>, textures: CanvasTextures) -> Self {
        Self {
            window,
            textures,
            is_cursor_locked: false,
            is_focused: false,
            frame_number: 0,
        }
    }

    /// Resize the canvas to fit the window surface
    pub fn resize(&mut self, width: u32, height: u32, context: &mut GpuContext) {
        if width > 0 && height > 0 {
            self.textures.resize(width, height, context);
        }
    }

    /// Reset the canvas window to match the configuration width and height
    pub fn reset(&mut self, context: &mut GpuContext) {
        self.resize(self.textures.config.width, self.textures.config.height, context);
    }

    /// Set the window cursor lock status
    pub fn set_cursor_lock(&mut self, lock: bool) {
        if lock {
            if self.window.set_cursor_grab(CursorGrabMode::Locked).is_ok() 
                || self.window.set_cursor_grab(CursorGrabMode::Confined).is_ok()
            {
                self.window.set_cursor_visible(false);
                self.is_cursor_locked = true;
            }
        } else {
            let _ = self.window.set_cursor_grab(CursorGrabMode::None);
            self.window.set_cursor_visible(true);
            self.is_cursor_locked = false;
        }
    }

    /// Retrieve the next canvas frame. This increments the frame counter.
    pub fn next_frame<'a>(&'a mut self) -> Result<CanvasFrame<'a>, wgpu::SurfaceError> {
        let surface = self.textures.next_surface_texture()?;
        let view = surface.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let curr_frame = self.frame_number;
        
        self.frame_number += 1;

        Ok(CanvasFrame { 
            surface,
            colors: &self.textures.colors,
            depths: &self.textures.depths,
            view, 
            number: curr_frame,
            dimensions: self.dimensions()
        })
    }

    /// Returns a `wgpu::ColorTargetState` as defined by the provided `RenderAttachment` implementation and state configurations, if available.
    pub fn color_state(
        &self,
        atm: impl RenderAttachment,
        blend: Option<wgpu::BlendState>,
        write_mask: wgpu::ColorWrites,
    ) -> Option<wgpu::ColorTargetState> {
        self.textures.color_state(atm, blend, write_mask)
    }

    /// Returns a `wgpu::DepthStencilState` as defined by the provided `RenderAttachment` implementation and state configurations, if available.
    pub fn depth_state(
        &self, 
        atm: impl RenderAttachment, 
        compare: wgpu::CompareFunction, 
        write_enabled: bool
    ) -> Option<wgpu::DepthStencilState>{
        self.textures.depth_state(atm, compare, write_enabled)
    }

    /// Returns the current dimensions of the canvas in pixels
    pub fn dimensions(&self) -> CanvasDimensions {
        self.textures.surface_dimensions()
    }

    /// Returns the current width of the canvas in pixels
    pub fn width(&self) -> u32 {
        self.dimensions().width
    }

    /// Returns the current height of the canvas in pixels
    pub fn height(&self) -> u32 {
        self.dimensions().height
    }

    /// Get the aspect ratio of the canvas
    pub fn aspect(&self) -> f32 {
        self.textures.aspect()
    }

    /// Get the number of frames presented since application start
    pub fn frame_count(&self) -> u32 {
        self.frame_number
    }

    /// Check if the canvas has the pointer focused on it.
    pub fn is_focused(&self) -> bool {
        self.is_focused
    }

    /// Check if the cursor is locked to the canvas
    pub fn cursor_locked(&self) -> bool {
        self.is_cursor_locked
    }
    
    /// request the window to trigger a redraw event
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Set the canvas to be in the focused state
    pub fn set_focused(&mut self, focused: bool) {
        self.is_focused = focused;
    }
}