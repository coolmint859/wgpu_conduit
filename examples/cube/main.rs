use std::{sync::Arc, time::Instant};

use wgpu_conduit::{DepthLoadOp, DrawCommand, Graphics, MultiBufferExecutor, StandardDepth, gpu::GraphicsInit};
use winit::{application::ApplicationHandler, dpi::{PhysicalSize, Size}, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow, EventLoop}, keyboard::{KeyCode, PhysicalKey}, window::{WindowAttributes, WindowId}};

mod cube;
mod camera;
use crate::{camera::Camera, cube::Cube};

pub struct App {
    graphics: Option<Graphics>,
    cube: Option<Cube>,
    camera: Camera,

    previous_time: Instant,
}

impl App {
    pub fn new() -> Self {
        App {
            graphics: None,
            cube: None,
            camera: Camera::new(glam::vec3(0.0, 0.0, 3.0)),
            previous_time: Instant::now()
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(graphics) = &mut self.graphics {
            graphics.resize_canvas(width, height);
        }
    }

    pub fn render(&mut self) {
        let Some(graphics) = &mut self.graphics else { return; };
        let Some(cube) = &self.cube else { return; };

        let frame = graphics.canvas.next_frame()
            .expect("Failed to aqcuire next canvas frame");

        let mut draw_cmd = DrawCommand::standard(
            &frame, 
            Some(wgpu::Color::BLACK), 
            StandardDepth,
            DepthLoadOp::Clear(1.0)
        );

        cube.render(&mut draw_cmd);
        MultiBufferExecutor::execute_draw(draw_cmd, &mut graphics.context);

        frame.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window_attrs = WindowAttributes::default()
            .with_title("Hello, Cube! Press T to Toggle Cube Texture Rendering")
            .with_inner_size(Size::Physical(PhysicalSize { 
                width: 1000, height: 1000,
            }));

        let window = Arc::new(
            event_loop.create_window(window_attrs).unwrap()
        );

        let mut graphics = GraphicsInit::new()
            .with_backend(wgpu::Backends::DX12)
            .with_depth_attachment(StandardDepth)
            .init_blocking(window)
            .expect("Failed to initialize graphics environment");

        self.camera.init(graphics.canvas.aspect(), &mut graphics.context);

        let cube = Cube::new(&mut graphics, self.camera.id);
        self.cube = Some(cube);

        self.graphics = Some(graphics);
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        let Some(graphics) = &mut self.graphics else { return; };
        let Some(cube) = &mut self.cube else { return; };

        let current_time = Instant::now();
        let dt = (current_time - self.previous_time).as_secs_f32();
        self.previous_time = current_time;

        self.camera.update(graphics.canvas.aspect(), &mut graphics.context);
        cube.update(dt, &mut graphics.context);

        graphics.sync_context();
        graphics.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::Resized(size) => self.resize(size.width, size.height),
            WindowEvent::KeyboardInput { event, ..} => {
                let Some(cube) = &mut self.cube else { return; };

                // only toggle if this is the first time the key was pressed
                if event.repeat || !event.state.is_pressed() { return; }
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::KeyT) => cube.switch(),
                    _ => {}
                }
            }
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