use std::{borrow::Cow, println};

use wgpu::util::DeviceExt;

use crate::{context::BindGroupLayoutHandle, blueprints::*};

/// A lightweight handle to core wgpu primitives, including the instance, adapter, device and queue.
/// 
/// This is the core struct used by the `GpuContext` to create resources.
/// 
/// While it's possible to use this to create resources directly, it's not recommended as the execution layer relies heavily
/// on the `GpuContext` and id system. Bind group and pipeline blueprints also rely heavily on the id system. All blueprints are capable of producing ids to their corresponding resources,
/// so creating the ids directly is feasible, but it's not the intended usage. You would need to handle the mapping of the ids to the resources yourself.
/// 
/// All in all, only use this directly to create resources if you're planning on handling the resource storage and execution layer yourself.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct GpuHandle {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl GpuHandle {
    pub fn limits(&self) -> wgpu::Limits {
        self.adapter.limits()
    }

    /// Create a buffer from the given configuration builder
    pub fn create_buffer(&self, blueprint: BufferBlueprint, init_data: Option<Vec<u8>>, capacity: u64) -> Result<wgpu::Buffer, String> {
        let payload = blueprint.into_payload(init_data, capacity);

        let buffer = if let Some(data) = payload.init_data {
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&payload.label),
                contents: &BufferBlueprint::extend_bytes(data, capacity as usize),
                usage: payload.usage
            })
        } else {
            self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&payload.label),
                size: payload.capacity,
                usage: payload.usage,
                mapped_at_creation: false
            })
        };

        println!("[GpuContext] Created new buffer with label '{}'", payload.label);

        Ok(buffer)
    }

    /// Create a new texture from the given configuration builder
    pub fn create_texture(&self, blueprint: TextureBlueprint, source: impl TextureSource) -> Result<TextureHandle, String> {
        let tex_data = match source.into_data(blueprint.format) {
            Ok(data) => data,
            Err(e) => {
                println!("An error occurred when getting texture data: {e}");
                return Err(e);
            }
        };
        let tex_payload = blueprint.into_payload(tex_data.data, tex_data.dim);

        let extent = wgpu::Extent3d {
            width: tex_payload.size.width,
            height: tex_payload.size.height,
            depth_or_array_layers: tex_payload.size.depth
        };
        
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&tex_payload.label),
            size: extent,
            mip_level_count: tex_payload.mip_levels,
            sample_count: 1,
            dimension: tex_payload.size.wgpu_dim,
            format: tex_payload.format,
            usage: tex_payload.usage,
            view_formats: &[],
        });

        if let Some(data) = &tex_payload.data {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                }, 
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(tex_payload.bytes_per_pixel() * tex_payload.size.width),
                    rows_per_image: Some(tex_payload.size.height)
                },
                extent,
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        println!("[GpuContext] Created new texture with label '{}'", tex_payload.label);

        Ok(TextureHandle { texture, view, extent })
    }

    /// Create a new sampler from the given configuration builder
    pub fn create_sampler(&self, blueprint: impl SamplerType) -> Result<wgpu::Sampler, String> {
        let payload = blueprint.into_payload()?;
        let sampler = self.device.create_sampler(&payload.desc);

        println!("[GpuContext] Created new sampler with label '{:?}'", payload.desc.label);

        Ok(sampler)
    }

    pub fn create_bg_layout(&self, blueprint: BindGroupBlueprint) -> Result<wgpu::BindGroupLayout, String> {
        let layout = self.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
            label: Some(&format!("Layout: {}", blueprint.label)),
            entries: &blueprint.layout.entries
        });

        println!("[GpuContext] Created new bind group layout with label '{}'", blueprint.label);

        Ok(layout)
    }

    /// Create a new bind group from the given configuration builder and resource map
    pub fn create_bind_group(
        &self,
        blueprint: BindGroupBlueprint, 
        entries: Vec<wgpu::BindGroupEntry<'_>>,
        layout: BindGroupLayoutHandle,
    ) -> Result<wgpu::BindGroup, String> {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&blueprint.label),
            layout: &layout,
            entries: &entries,
        });

        println!("[GpuContext] Created new bind group with label '{}'", blueprint.label);

        Ok(bind_group)
    }

    /// Create a new render pipeline from the given configuration builder
    pub fn create_render_pipeline(
        &self,
        blueprint: PipelineBlueprint<Render>,
        bg_layouts: Vec<wgpu::BindGroupLayout>
    ) -> Result<PipelineHandle, String> {
        let shader_source = match blueprint.shader {
            ShaderSource::OnDisk(path) => {
                match std::fs::read_to_string(&path) {
                    Ok(source) => source,
                    Err(e) => {
                        println!("[Render Pipeline] Failed to read shader file '{}': {e}", path);
                        return Err(format!("[Render Pipeline] Failed to read shader file '{}': {e}", path));
                    }
                }
            },
            ShaderSource::Embedded(source) => source.to_string()
        };

        let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&blueprint.label),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_source))
        });

        let bg_layout_refs: Vec<&wgpu::BindGroupLayout> = bg_layouts
            .iter()
            .map(|layout| layout)
            .collect();

        let layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{}_layout", blueprint.label)),
            bind_group_layouts: &bg_layout_refs,
            immediate_size: 0,
        });

        let vertex_layouts: Vec<_> = blueprint.ty.vertex_layouts
            .iter()
            .map(|l| l.as_wgpu_layout())
            .collect();

        let pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&blueprint.label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some(&blueprint.ty.vs_main),
                compilation_options: Default::default(),
                buffers: &vertex_layouts,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(&blueprint.ty.fs_main),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: blueprint.ty.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: blueprint.ty.primitive_state,
            depth_stencil: blueprint.ty.depth_stencil,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        println!("[GpuContext] Created new render pipeline with label '{}'", blueprint.label);

        Ok(PipelineHandle::Render(pipeline))
    }

    pub fn create_compute_pipeline(
        &self, 
        blueprint: PipelineBlueprint<Compute>,
        bg_layouts: Vec<wgpu::BindGroupLayout>
    ) -> Result<PipelineHandle, String> {
        let shader_source = match blueprint.shader {
            ShaderSource::OnDisk(path) => {
                match std::fs::read_to_string(&path) {
                    Ok(source) => source,
                    Err(e) => {
                        println!("[Render Pipeline] Failed to read shader file '{}': {e}", path);
                        return Err(format!("[Render Pipeline] Failed to read shader file '{}': {e}", path));
                    }
                }
            },
            ShaderSource::Embedded(source) => source.to_string()
        };

        let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{}_source", blueprint.label)),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_source))
        });

        let bg_layout_refs: Vec<&wgpu::BindGroupLayout> = bg_layouts
            .iter()
            .map(|layout| layout)
            .collect();

        let layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{} Layout", blueprint.label)),
            bind_group_layouts: &bg_layout_refs,
            immediate_size: 0,
        });

        let pipeline = self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(&blueprint.label),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some(&blueprint.ty.main),
            compilation_options: Default::default(),
            cache: None
        });

        println!("[GpuContext] Created new compute pipeline with label '{}'", blueprint.label);

        Ok(PipelineHandle::Compute(pipeline))
    }
}