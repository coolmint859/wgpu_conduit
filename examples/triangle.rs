use std::sync::Arc;

use wgpu_conduit::{Canvas, DrawCommand, MultiBufferExecutor, PipelineBlueprint, blueprints::{BufferBlueprint, BufferSource, ShaderSource, VertexBufferLayout, VertexLayout}, context::{BufferId, GpuContext, PipelineId}, execution::IndexedDraw};
use winit::{application::ApplicationHandler, dpi::{PhysicalSize, Size}, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, window::{WindowAttributes, WindowId}};

/// Container for the gpu context and rendering canvas
pub struct Graphics {
    pub context: GpuContext,
    pub canvas: Canvas,
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}

impl VertexLayout for Vertex {
    fn into_layout() -> VertexBufferLayout {
        VertexBufferLayout::as_vertex_step(0)
            .with_attribute(wgpu::VertexFormat::Float32x3)
            .with_attribute(wgpu::VertexFormat::Float32x3)
    }
}

pub struct Triangle {
    vertices: BufferId,
    indices: BufferId,
    pipeline: PipelineId,
} 

impl Triangle {
    pub fn new(context: &mut GpuContext) -> Self {
        let vertices = context.request_buffer(
            BufferBlueprint::as_vertex(), 
            BufferSource::from_vec(vec![
                Vertex { position: [ 0.0,  0.5, 0.0], color: [1.0, 0.0, 0.0] },
                Vertex { position: [-0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
                Vertex { position: [ 0.5, -0.5, 0.0], color: [0.0, 0.0, 1.0] },
            ])
        );

        let indices = context.request_buffer(
            BufferBlueprint::as_index(), 
            BufferSource::from_vec(vec![0u32, 1, 2])
        );

        let shader = ShaderSource::Embedded(include_str!("shaders/2d_render.wgsl"));
        let pipeline = context.request_pipeline(
            PipelineBlueprint::as_render(shader)
                .with_vertex_layout::<Vertex>()
        );

        Self {
            vertices,
            indices,
            pipeline,
        }
    }

    pub fn render(&self, draw_cmd: &mut DrawCommand) {
        let draw = IndexedDraw::new(self.pipeline, 0..3)
            .with_vertex_buffers(&[self.vertices])
            .with_index_buffer(self.indices, wgpu::IndexFormat::Uint32);

        draw_cmd.add_draw(draw);
    }
}

pub struct App {
    graphics: Option<Graphics>,
    triangle: Option<Triangle>,
}

impl App {
    pub fn new() -> Self {
        App {
            graphics: None,
            triangle: None,
        }
    }

    pub fn render(&mut self) {
        let Some(graphics) = &mut self.graphics else { return; };
        let Some(triangle) = &self.triangle else { return; };

        let frame = graphics.canvas.next_frame()
            .expect("Failed to get next frame.");

        let mut draw_cmd = DrawCommand::standard_no_depth(
            &frame, 
            Some(wgpu::Color::BLACK)
        );

        triangle.render(&mut draw_cmd);
        MultiBufferExecutor::execute_draw(draw_cmd, &mut graphics.context);

        frame.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_dims = Size::Physical(PhysicalSize { width: 1000, height: 1000 });

        let window_attrs = WindowAttributes::default()
            .with_inner_size(window_dims)
            .with_title("Hello, Triangle!");

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());

        let mut context = GpuContext::builder()
            .with_backend(wgpu::Backends::DX12)
            .init_blocking()
            .expect("Failed to initialize gpu context");

        let canvas = Canvas::builder()
            .init(window, &context)
            .expect("Failed to initialize rendering canvas");

        let triangle = Triangle::new(&mut context);

        self.graphics = Some(Graphics { context, canvas });
        self.triangle = Some(triangle);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let Some(graphics) = &mut self.graphics else { return; };

        graphics.context.sync();
        graphics.canvas.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.render(),
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    env_logger::init();

    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}