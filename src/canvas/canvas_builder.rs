use std::{collections::HashMap, sync::Arc};

use winit::window::Window;

use crate::{GpuContext, canvas::{Canvas, FrameBuffer, RenderAttachment, RenderAttachmentConfig, RenderAttachmentRecord}};

/// Helper struct to generate new rendering canvases.
pub struct CanvasBuilder {
    attachments: HashMap<String, RenderAttachmentConfig>,
    present_mode: wgpu::PresentMode,
    frame_latency: u32,
}

impl CanvasBuilder {
    pub fn new() -> Self {
        Self {
            attachments: HashMap::new(),
            present_mode: wgpu::PresentMode::Fifo,
            frame_latency: 2,
        }
    }

    /// Set the presentation mode for the rendering surface
    pub fn with_present_mode(mut self, present_mode: wgpu::PresentMode) -> Self {
        self.present_mode = present_mode;
        self
    }

    /// Set the max frame latency between surface texture requests and surface presentation.
    pub fn with_max_latency(mut self, latency: u32) -> Self {
        self.frame_latency = latency;
        self
    }

    /// Add a render attachment to the `Canvas`.
    pub fn with_attachment(mut self, atm: impl RenderAttachment) -> Self {
        self.attachments.insert(atm.name().to_string(), atm.into_config());
        self
    }

    /// Initialize the `Canvas` with the provided `window`. Returns a ready-to-use `Canvas` instance.
    /// 
    /// The `Canvas` is forever joined with the `window` when this method is called. That is, rendering with this canvas will be presented to that window. 
    pub fn init(self, window: Arc<Window>, context: &GpuContext) -> Result<Canvas, wgpu::CreateSurfaceError> {
        let size = window.inner_size();
        
        let surface = context.gpu.instance.create_surface(window.clone())?;
        let surface_caps = surface.get_capabilities(&context.gpu.adapter);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_caps.formats[0],
            width: size.width,
            height: size.height,
            present_mode: self.present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: self.frame_latency,
        };
        surface.configure(&context.gpu.device, &config);

        let attachments = self.attachments
            .into_iter()
            .map(|(key, config)| {
                let record = RenderAttachmentRecord {
                    tex: config.create_handle(size.width, size.height, context), 
                    config
                };

                (key.clone(), record)
            })
            .collect();

        let frame_buffer = FrameBuffer { 
            surface, 
            config, 
            attachments 
        };

        Ok(Canvas::new(window, frame_buffer))
    }
}