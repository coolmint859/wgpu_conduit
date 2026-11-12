#![allow(dead_code)]
use std::{collections::HashMap, sync::Arc};
use winit::window::Window;

use crate::{AttachmentConfig, AttachmentRecord, Canvas, CanvasTextures, RenderAttachment, gpu::GpuHandle, context::{GlobalHoldPolicy, GpuContext}};

/// Graphics initialization errors
#[derive(Clone, Debug)]
pub enum GraphicsInitError {
    /// Surface Failed to be created
    Surface(wgpu::CreateSurfaceError),
    /// Adapter failed requesteding
    Adapter(wgpu::RequestAdapterError),
    /// Logical Device failed requesting
    Device(wgpu::RequestDeviceError),
    /// Graphics have already been initialized
    Initialized,
}

impl From<wgpu::CreateSurfaceError> for GraphicsInitError {
    fn from(err: wgpu::CreateSurfaceError) -> Self {
        GraphicsInitError::Surface(err)
    }
}

impl From<wgpu::RequestAdapterError> for GraphicsInitError {
    fn from(err: wgpu::RequestAdapterError) -> Self {
        GraphicsInitError::Adapter(err)
    }
}

impl From<wgpu::RequestDeviceError> for GraphicsInitError {
    fn from(err: wgpu::RequestDeviceError) -> Self {
        GraphicsInitError::Device(err)
    }
}

pub struct Graphics {
    pub context: GpuContext,
    pub canvas: Canvas,
}

impl Graphics {
    pub fn resize_canvas(&mut self, width: u32, height: u32) {
        self.canvas.resize(width, height, &mut self.context);
    }

    pub fn request_redraw(&self) {
        self.canvas.request_redraw();
    }

    pub fn sync_context(&mut self) {
        self.context.sync();
    }
}

/// Initializes the graphics environment for a given window
pub struct GraphicsInit {
    power_pref: wgpu::PowerPreference,
    present_mode: wgpu::PresentMode,
    frame_latency: u32,
    back_end: wgpu::Backends,

    global_hp: GlobalHoldPolicy,
    
    color_attachments: HashMap<String, AttachmentConfig>,
    depth_attachments: HashMap<String, AttachmentConfig>,
    
    is_initialized: bool
}

impl GraphicsInit {
    pub fn new() -> Self {
        Self {
            power_pref: wgpu::PowerPreference::None,
            present_mode: wgpu::PresentMode::Immediate,
            frame_latency: 2,
            back_end: wgpu::Backends::PRIMARY,
            global_hp: GlobalHoldPolicy::default(),
            color_attachments: HashMap::new(),
            depth_attachments: HashMap::new(),
            is_initialized: false
        }
    }

    /// Set the power preference for the instance adapter
    pub fn with_power_pref(mut self, power_pref: wgpu::PowerPreference) -> Self {
        self.power_pref = power_pref;
        self
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

    /// Set the target graphics api backend. The default is wgpu::Backends::PRIMARY, which is usually Vulkan.
    /// However it may be worth it to consider setting this to a platform specific backend (such as DX12) depending on your application needs.
    pub fn with_backend(mut self, backend: wgpu::Backends) -> Self {
        self.back_end = backend;
        self
    }

    /// Set the global hold policy for resources created with the gpu
    pub fn with_global_hp(mut self, policy: GlobalHoldPolicy) -> Self {
        self.global_hp = policy;
        self
    }

    /// Add a color attachment. These are forwarded to the `Canvas` during initialization to be used during rendering.
    pub fn with_color_attachment(mut self, atm: impl RenderAttachment) -> Self {
        self.color_attachments.insert(atm.name().to_string(), atm.into_config());
        self
    }

    /// Add a depth attachment. These are forwarded to the `Canvas` during initialization to be used during rendering.
    /// 
    /// Note that `Pipelines` and `DrawCommands` can only be configured with one depth attachment at a time.
    pub fn with_depth_attachment(mut self, atm: impl RenderAttachment) -> Self {
        self.depth_attachments.insert(atm.name().to_string(), atm.into_config());
        self
    }

    /// Initialize the graphics environment with the provided window. Consumes `self`
    /// 
    /// unlike `init()`, this method blocks the calling thread until complete.
    pub fn init_blocking(self, window: Arc<Window>) -> Result<Graphics, GraphicsInitError> {
        pollster::block_on(self.init(window))
    }

    /// Inititialize the graphics environment with the provided window. Consumes `self`
    pub async fn init(self, window: Arc<Window>) -> Result<Graphics, GraphicsInitError> {
        if self.is_initialized { return Err(GraphicsInitError::Initialized) }

        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: self.back_end,
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: self.power_pref,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;
        let gpu = GpuHandle { device, queue };

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: self.present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: self.frame_latency,
        };
        surface.configure(&gpu.device, &config);

        let mut context = GpuContext::new(gpu, self.global_hp);
        
        let colors = self.color_attachments
            .iter()
                .map(|(atm_name, config)| {
                let name = atm_name.clone();
                let tex = config.create_handle(size.width, size.height, &mut context);
                let record = AttachmentRecord { config: config.clone(), tex };

                (name, record)
            })
            .collect();

        let depths = self.depth_attachments
            .iter()
            .map(|(atm_name, config)| {
                let name = atm_name.clone();
                let tex = config.create_handle(size.width, size.height, &mut context);
                let record = AttachmentRecord { config: config.clone(), tex };

                (name, record)
            })
            .collect();
        
        let canvas = Canvas::new(window, CanvasTextures {
            surface, colors, depths, config
        });

        Ok(Graphics { context, canvas })
    }
}