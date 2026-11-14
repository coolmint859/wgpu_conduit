use std::collections::HashMap;

use crate::{ColorAttachment, DepthAttachment, DepthLoadOp, canvas::{CanvasDimensions, RenderAttachment, RenderAttachmentRecord, SurfaceHandle}};

/// The current rendering frame.
pub struct CanvasFrame<'a> {
    /// The rendering surface
    pub(crate) surface: SurfaceHandle,
    /// known color attachments
    pub(crate) attachments: &'a HashMap<String, RenderAttachmentRecord>,
    /// The current frame number
    pub(crate) number: u32,
    /// the width/height of the `Canvas`
    pub(crate) dimensions: CanvasDimensions
}

impl<'a> CanvasFrame<'a> {
    /// Returns the width of the `CanvasFrame`.
    pub fn width(&self) -> u32{
        self.dimensions.width
    }

    /// Returns the width of the `CanvasFrame`.
    pub fn height(&self) -> u32 {
        self.dimensions.height
    }

    /// Returns the dimensions of the `CanvasFrame`
    pub fn dimensions(&self) -> CanvasDimensions {
        self.dimensions
    }

    /// Returns this `CanvasFrame`'s number. (i.e the number of `CanvasFrame`s presented since the application launched)
    pub fn number(&self) -> u32 {
        self.number
    }

    /// Returns the aspect ratio of the `CanvasFrame`.
    pub fn aspect(&self) -> f32 {
        self.dimensions.width as f32 / self.dimensions.height as f32
    }

    /// Present the `CanvasFrame` to the surface texture. This consumes the frame.
    pub fn present(self) {
        self.surface.texture.present();
    }

    /// Returns the current surface texture view.
    pub fn surface_view(&self) -> wgpu::TextureView {
        self.surface.view.clone()
    }

    // /// Returns a target texture view as defined by the provided `RenderAttachment` implementation, if available.
    // pub fn get_target_view(&self, atm: impl RenderAttachment) -> Option<wgpu::TextureView> {
    //     let name = atm.name();
    //     self.attachments
    //         .get(name)
    //         .map(|record| {
    //             record.tex.view.clone()
    //         })
    // }

    /// Returns the current surface texture view as a `ColorAttachment`
    pub fn surface_attachment(&self, clear_color: Option<wgpu::Color>) -> ColorAttachment {
        ColorAttachment {
            view: self.surface.view.clone(),
            clear_color,
        }
    }

    /// Returns a color attachment as defined by the provided `RenderAttachment` implementation, if available.
    pub fn color_attachment(&self, atm: impl RenderAttachment, clear_color: Option<wgpu::Color>) -> Option<ColorAttachment> {
        let record = self.attachments.get(atm.name())?;

        Some(ColorAttachment {
            view: record.tex.view.clone(),
            clear_color
        })
    }

    /// Returns a depth attachment as defined by the provided `RenderAttachment` implementation, if available.
    pub fn depth_attachment(&self, atm: impl RenderAttachment, load_op: DepthLoadOp) -> Option<DepthAttachment> {
        let record = self.attachments.get(atm.name())?;

        Some(DepthAttachment {
            view: record.tex.view.clone(),
            op: load_op
        })
    }
}