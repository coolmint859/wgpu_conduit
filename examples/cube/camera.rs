use wgpu_conduit::{BufferBlueprint, BufferId, BufferSource, StructuredUpdate};

use crate::Graphics;

/// A simple 3d camera
pub struct Camera {
    position: glam::Vec3,
    fov: f32,
    z_near: f32,
    z_far: f32,

    pub id: BufferId,
}

impl Camera {
    pub fn new(position: glam::Vec3) -> Self {
        Self {
            position,
            fov: 60f32.to_radians(),
            z_near: 0.01,
            z_far: 100.0,
            id: BufferId::UNINIT
        }
    }

    /// Initialize this camera
    pub fn init(&mut self, graphics: &mut Graphics) {
        self.id = graphics.context.request_buffer(
            BufferBlueprint::uniform()
                .writable()
                .with_label("camera_uniform"), 
            BufferSource::from_pod(
                self.view_proj(graphics.canvas.aspect())
            )
        );
    }

    /// Update this camera
    pub fn update(&mut self, graphics: &mut Graphics) {
        let _ = graphics.context.update_buffer(&self.id, StructuredUpdate {
            data: &self.view_proj(graphics.canvas.aspect()),
            offset: 0
        });
    }

    /// Returns this camera's view-projection matrix
    pub fn view_proj(&self, aspect: f32) -> glam::Mat4 {
        let view = glam::Mat4::from_translation(self.position).inverse();
        let proj = glam::Mat4::perspective_rh(
            self.fov, 
            aspect, 
            self.z_near, 
            self.z_far
        );

        proj * view
    }
}