use async_store::HoldPolicy;

use crate::{GpuContext, gpu::GpuHandle};

/// Graphics initialization errors
#[derive(Clone, Debug)]
pub enum GpuInitError {
    /// Adapter failed requesting
    Adapter(wgpu::RequestAdapterError),
    /// Logical Device failed requesting
    Device(wgpu::RequestDeviceError),
}

impl From<wgpu::RequestAdapterError> for GpuInitError {
    fn from(err: wgpu::RequestAdapterError) -> Self {
        GpuInitError::Adapter(err)
    }
}

impl From<wgpu::RequestDeviceError> for GpuInitError {
    fn from(err: wgpu::RequestDeviceError) -> Self {
        GpuInitError::Device(err)
    }
}

/// The `HoldPolicy` for all gpu resources globally. All resources have an indefinite hold policy by default.
/// 
/// These are followed internally by the `GpuContext` unless a resource blueprint is configured otherwise.
#[derive(Debug, Clone, Copy)]
pub struct GlobalHoldPolicy {
    pub buffers: HoldPolicy,
    pub textures: HoldPolicy,
    pub samplers: HoldPolicy,
    pub bind_groups: HoldPolicy,
    pub pipelines: HoldPolicy,
}

impl Default for GlobalHoldPolicy {
    fn default() -> Self {
        Self {
            buffers: HoldPolicy::Indefinite,
            textures: HoldPolicy::Indefinite,
            samplers: HoldPolicy::Indefinite,
            bind_groups: HoldPolicy::Indefinite,
            pipelines: HoldPolicy::Indefinite,
        }
    }
}

/// Initialization struct for the gpu. This is returned from `GpuContext::builder()`.
/// 
/// You can use this to configure the gpu before initializing it.
pub struct ContextBuilder {
    power_pref: wgpu::PowerPreference,
    back_end: wgpu::Backends,
    global_hp: GlobalHoldPolicy,
}

impl ContextBuilder {
    /// Create an initialization struct for the gpu. 
    /// 
    /// You typically only need to call this once per application.
    pub fn new() -> Self {
        Self {
            power_pref: wgpu::PowerPreference::None,
            back_end: wgpu::Backends::PRIMARY,
            global_hp: GlobalHoldPolicy::default()
        }
    }

    /// Set the global hold policy for gpu resources created through the `GpuContext`.
    pub fn with_global_policy(mut self, policy: GlobalHoldPolicy) -> Self {
        self.global_hp = policy;
        self
    }

    /// Set the power preference for the gpu adapter
    pub fn with_power_pref(mut self, power_pref: wgpu::PowerPreference) -> Self {
        self.power_pref = power_pref;
        self
    }

    /// Set the target graphics api backend. The default is wgpu::Backends::PRIMARY, which is usually Vulkan.
    /// However it may be worth it to consider setting this to a platform specific backend (such as DX12) depending on your application needs.
    pub fn with_backend(mut self, backend: wgpu::Backends) -> Self {
        self.back_end = backend;
        self
    }

    /// Initialize the gpu environment. Returns a ready-to-use `GpuContext` upon completion.
    /// 
    /// Unlike `init()`, this method blocks the calling thread until complete.
    pub fn init_blocking(self) -> Result<GpuContext, GpuInitError> {
        pollster::block_on(self.init())
    }

    /// Initialize the gpu environment. Returns a ready to use `GpuContext` upon completion.
    pub async fn init(self) -> Result<GpuContext, GpuInitError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: self.back_end,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: self.power_pref,
                compatible_surface: None,
                force_fallback_adapter: false,
            }).await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;

        let gpu = GpuHandle { instance, adapter, device, queue };

        Ok(GpuContext::new(gpu, self.global_hp))
    }
}