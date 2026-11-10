use wgpu_conduit::{Buffer, BufferId, BufferSource, GpuContext, StructuredUpdate};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_proj: [f32; 16]
}

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
    pub fn init(&mut self, aspect: f32, context: &mut GpuContext) {
        self.id = context.request_buffer(
            Buffer::as_uniform().writable().with_label("camera_uniform"), 
            BufferSource::from_pod(self.as_uniform(aspect))
        );
    }

    /// Update this camera
    pub fn update(&mut self, aspect: f32, context: &mut GpuContext) {
        let _ = context.update_buffer(&self.id, StructuredUpdate {
            data: &self.as_uniform(aspect),
            offset: 0
        });
    }

    /// Returns this camera as in it's uniform representation
    pub fn as_uniform(&self, aspect: f32) -> CameraUniform {
        let view = glam::Mat4::from_translation(self.position).inverse();
        let proj = glam::Mat4::perspective_rh(
            self.fov, 
            aspect, 
            self.z_near, 
            self.z_far
        );

        CameraUniform { view_proj: (proj * view).to_cols_array() }
    }
}