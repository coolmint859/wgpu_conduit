use std::sync::Arc;
use winit::window::{CursorGrabMode, Window};

use crate::{CanvasBuilder, CanvasFrame, FrameBuffer, RenderAttachment, context::GpuContext};

/// The width and height of the `Canvas`
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct CanvasDimensions {
    pub width: u32,
    pub height: u32
}

/// Represents the window and rendering surface
pub struct Canvas {
    pub(crate) window: Arc<Window>,
    pub frame_buffer: FrameBuffer,

    is_cursor_locked: bool,
    is_focused: bool,
    frame_number: u32,
}

impl Canvas {
    /// Create a new `Canvas` from a builder.
    pub fn builder() -> CanvasBuilder {
        CanvasBuilder::new()
    }

    /// Creates a new canvas given a winit `Window` and `FrameBuffer`
    pub(crate) fn new(window: Arc<Window>, frame_buffer: FrameBuffer) -> Self {
        Self {
            window,
            frame_buffer,
            is_cursor_locked: false,
            is_focused: false,
            frame_number: 0,
        }
    }

    /// Resize the canvas to fit the window surface
    pub fn resize(&mut self, width: u32, height: u32, context: &GpuContext) {
        if width > 0 && height > 0 {
            self.frame_buffer.resize(width, height, context);
        }
    }

    /// Reset the canvas window to match the configuration width and height
    pub fn reset(&mut self, context: &mut GpuContext) {
        self.resize(self.frame_buffer.config.width, self.frame_buffer.config.height, context);
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
        let curr_frame = self.frame_number;
        self.frame_number += 1;

        Ok(CanvasFrame { 
            surface: self.frame_buffer.swap_surface()?,
            attachments: &self.frame_buffer.attachments,
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
        self.frame_buffer.color_state(atm, blend, write_mask)
    }

    /// Returns a `wgpu::DepthStencilState` as defined by the provided `RenderAttachment` implementation and state configurations, if available.
    pub fn depth_state(
        &self, 
        atm: impl RenderAttachment, 
        compare: wgpu::CompareFunction, 
        write_enabled: bool
    ) -> Option<wgpu::DepthStencilState>{
        self.frame_buffer.depth_state(atm, compare, write_enabled)
    }

    /// Returns the current dimensions of the canvas in pixels
    pub fn dimensions(&self) -> CanvasDimensions {
        self.frame_buffer.surface_dimensions()
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
        self.frame_buffer.surface_aspect()
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