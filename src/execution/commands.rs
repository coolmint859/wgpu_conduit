use core::panic;

use crate::{CanvasFrame, RenderAttachment, execution::DrawCall, gpu_state::{BindGroupId, GpuContext, PipelineId}};
/// Represents commands that can submitted and recorded by an Executor implementation.
pub trait GpuCommand {
    /// Record this command using the provided encoder in the provided context
    fn record(&mut self, encoder: &mut wgpu::CommandEncoder, context: &GpuContext);
}

/// A color attachment for use in a `DrawCommand`.
pub struct ColorAttachment {
    pub view: wgpu::TextureView,
    pub clear_color: Option<wgpu::Color>
}

impl ColorAttachment {
    /// Converts `self` into a `wgpu::RenderPassColorAttachment`
    pub fn into_desc<'a>(&'a self) -> wgpu::RenderPassColorAttachment<'a> {
        let load_op = match self.clear_color {
            Some(color) => wgpu::LoadOp::Clear(color),
            None => wgpu::LoadOp::Load,
        };
        
        wgpu::RenderPassColorAttachment {
            view: &self.view,
            ops: wgpu::Operations {
                load: load_op,
                store: wgpu::StoreOp::Store,
            },
            resolve_target: None,
            depth_slice: None
        }
    }
}

/// A depth attachment for use in a `DrawCommand`.
pub struct DepthAttachment {
    pub view: wgpu::TextureView,
    pub op: DepthLoadOp
}

impl DepthAttachment {
    /// Converts `self` into a `wgpu::RenderPassDepthStencilAttachment`
    pub fn into_desc<'a>(&'a self) -> wgpu::RenderPassDepthStencilAttachment<'a> {
        let load_op = match self.op {
            DepthLoadOp::Clear(value) => Some(wgpu::LoadOp::Clear(value)),
            DepthLoadOp::Load => Some(wgpu::LoadOp::Load),
            DepthLoadOp::None => None,
        };

        let depth_ops = load_op.map(|op| {
            wgpu::Operations {
                load: op,
                store: wgpu::StoreOp::Store
            }
        });

        wgpu::RenderPassDepthStencilAttachment {
            view: &self.view,
            depth_ops,
            stencil_ops: None
        }
    }
}

/// The load operation for a depth attachment.
pub enum DepthLoadOp {
    /// Clear the depth texture to the provided value (typically 1.0)
    Clear(f32),
    /// Load in the current contents of the depth texture
    Load,
    /// No depth operations.
    None,
}

/// Renders `Drawable` entites to an output view via a wgpu render pass
pub struct DrawCommand {
    /// The set of draw calls to issue when `record()` is called.
    draw_calls: Vec<Box<dyn DrawCall>>,
    color_attachments: Vec<ColorAttachment>,
    depth_attachment: Option<DepthAttachment>,
}

impl DrawCommand {
    /// Create a new draw command
    pub fn new() -> Self {
        Self {
            draw_calls: Vec::new(),
            color_attachments: Vec::new(),
            depth_attachment: None
        }
    }

    /// Create a `DrawCommand` for a standard main pass (with surface color attachment and depth attachment)
    /// 
    /// ## PANICS 
    /// 
    /// If the `CanvasFrame` does not have a depth attachment
    pub fn standard<'a>(
        frame: &'a CanvasFrame<'a>, 
        clear_color: Option<wgpu::Color>,
        depth_atm: impl RenderAttachment,
        depth_op: DepthLoadOp,
    ) -> Self {
        let surface_atm = frame.surface_attachment(clear_color);
        let depth_atm = match frame.depth_attachment(depth_atm, depth_op) {
            Some(depth) => depth,
            None => { panic!("CanvasFrame does not have a depth attachment.");}
        };

        let draw_cmd = DrawCommand::new()
            .with_color_atm(surface_atm)
            .with_depth_atm(depth_atm);

        draw_cmd
    }

    /// Create a `DrawCommand` for a standard main pass (with just surface color attachment, no depth)
    pub fn standard_no_depth<'a>(frame: &'a CanvasFrame<'a>, clear_color: Option<wgpu::Color>) -> Self {
        let surface_atm = frame.surface_attachment(clear_color);
        let draw_cmd = DrawCommand::new()
            .with_color_atm(surface_atm);

        draw_cmd
    }

    pub fn with_draws(mut self, drawables: impl IntoIterator<Item = impl DrawCall + 'static>) -> Self {
        for draw in drawables {
            self.draw_calls.push(Box::new(draw));
        }
        self
    }

    /// Add a Drawable item into the draw command
    pub fn with_draw(mut self, drawable: impl DrawCall + 'static) -> Self {
        self.add_draw(drawable);
        self
    }

    /// Add a Drawable item into the draw command
    pub fn add_draw(&mut self, drawable: impl DrawCall + 'static) {
        self.draw_calls.push(Box::new(drawable));
    }

    /// Returns true if at least one drawable was added to the command, false otherwise
    pub fn has_draws(&self) -> bool {
        return !self.draw_calls.is_empty()
    }

    /// Add a color attachment to the draw command
    pub fn with_color_atm(mut self, color: ColorAttachment) -> Self {
        self.color_attachments.push(color);
        self
    }

    /// Add a depth attachment to the draw command
    pub fn with_depth_atm(mut self, depth: DepthAttachment) -> Self {
        self.depth_attachment = Some(depth);
        self
    }
}

impl GpuCommand for DrawCommand {
    fn record(&mut self, encoder: &mut wgpu::CommandEncoder, context: &GpuContext) {
        let mut color_attachments = Vec::new();
        for atm in &mut self.color_attachments {
            color_attachments.push(Some(atm.into_desc()))
        };

        let depth_stencil_attachment = match &self.depth_attachment {
            Some(atm) => Some(atm.into_desc()),
            None => None
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &color_attachments,
            depth_stencil_attachment,
            ..Default::default()
        });

        for mut drawable in std::mem::take(&mut self.draw_calls) {
            drawable.invoke(&mut render_pass, context);
        }
    }
}

/// Dispatches a compute shader to the gpu
pub struct ComputeCommand {
    pip_id: PipelineId,
    bind_groups: Vec<BindGroupId>,
    workgroups: (u32, u32, u32),
}

impl ComputeCommand {
    pub fn new(pip_id: PipelineId, workgroups: (u32, u32, u32)) -> Self {
        Self {
            pip_id,
            workgroups,
            bind_groups: Vec::new()
        }
    }

    /// Add a set of bind groups to the draw command
    pub fn with_bind_groups(mut self, bind_groups: &[BindGroupId]) -> Self {
        self.bind_groups.extend_from_slice(bind_groups);
        self
    }
}

impl GpuCommand for ComputeCommand {
    fn record(&mut self, encoder: &mut wgpu::CommandEncoder, context: &GpuContext) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Pass"),
            ..Default::default()
        });

        let Some(pipeline) = context.get_pipeline(&self.pip_id).and_then(|pip| pip.to_compute()) else {
            // println!("[ComputeCommand] Failed to validate compute pipeline @{:?}", info.pipeline_id);
            return; 
        };
        compute_pass.set_pipeline(&pipeline);

        for (idx, bg_id) in self.bind_groups.iter().enumerate() {
            let Some(bg) = context.get_bind_group(bg_id) else { 
                // println!("[ComputeCommand] Failed to validate bind group @{:?} for compute pipeline @{:?}", bg_id, info.pipeline_id);
                return; 
            };
            compute_pass.set_bind_group(idx as u32, &bg.bind_group, &[]);
        }

        let (wx, wy, wz) = self.workgroups;
        compute_pass.dispatch_workgroups(wx, wy, wz);
    }
}