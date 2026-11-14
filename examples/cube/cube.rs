use wgpu_conduit::{BindGroupBlueprint, BindGroupIdPair, BufferBinding, BufferBlueprint, BufferId, BufferSource, DrawCommand, IndexedDraw, PipelineBlueprint, PipelineId, Procedural, SampledTextureOptions, Sampler, SamplerBinding, ShaderSource, StandardDepth, StructuredUpdate, TexDimensions, TextureBinding, TextureBlueprint, VertexBufferLayout, VertexLayout};

use crate::Graphics;

/// A vertex with a position and color attribute
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

impl VertexLayout for ColorVertex {
    fn into_layout() -> VertexBufferLayout {
        VertexBufferLayout::as_vertex_step(0)
            .with_attribute(wgpu::VertexFormat::Float32x3)
            .with_attribute(wgpu::VertexFormat::Float32x3)
    }
}

/// A vertex with a position and uv coordinate attribute
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextureVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

impl VertexLayout for TextureVertex {
    fn into_layout() -> VertexBufferLayout {
        VertexBufferLayout::as_vertex_step(0)
            .with_attribute(wgpu::VertexFormat::Float32x3)
            .with_attribute(wgpu::VertexFormat::Float32x2)
    }
}

pub struct Cube {
    colored: ColoredCube,
    textured: TexturedCube,
    render_textured: bool,

    model_mat: BufferId,
    et: f32,
}

impl Cube {
    pub fn new(graphics: &mut Graphics, cam_id: BufferId) -> Self {
        let model_mat = graphics.context.request_buffer(
            BufferBlueprint::as_uniform().writable().with_label("model_matrix"),
            BufferSource::from_pod(glam::Mat4::default())
        );

        Self {
            colored: ColoredCube::new(graphics, cam_id, model_mat),
            textured: TexturedCube::new(graphics, cam_id, model_mat),
            render_textured: false,
            model_mat,
            et: 0.0,
        }
    }

    pub fn switch(&mut self) {
        self.render_textured = !self.render_textured;
    }

    pub fn update(&mut self, dt: f32, graphics: &mut Graphics) {
        self.et += dt;

        let speed = 0.5;
        let x = speed * self.et;
        let y = speed * self.et;
        let mat_data = glam::Mat4::from_euler(glam::EulerRot::ZXY, x, y, 0.0);

        let _ = graphics.context.update_buffer(&self.model_mat, StructuredUpdate {
            data: &mat_data,
            offset: 0
        });
    }

    pub fn render(&self, draw_cmd: &mut DrawCommand) {
        if self.render_textured {
            self.textured.render(draw_cmd);
        } else {
            self.colored.render(draw_cmd);
        }
    }
}

pub struct ColoredCube {
    vt_buffer: BufferId,
    idx_buffer: BufferId,

    pipeline: PipelineId,
    bg: BindGroupIdPair,
}

impl ColoredCube {
    pub fn new(graphics: &mut Graphics, cam_id: BufferId, model_mat: BufferId) -> Self {
        let (vertices, indices) = create_colored_vertices();
        let vt_buffer = graphics.context.request_buffer(
            BufferBlueprint::as_vertex().with_label("cube_colored_vertices"), 
            BufferSource::from_vec(vertices)
        );

        let idx_buffer = graphics.context.request_buffer(
            BufferBlueprint::as_index().with_label("cube_colored_indices"),
            BufferSource::from_vec(indices)
        );

        let bg = graphics.context.request_bind_group(
            BindGroupBlueprint::new()
                .with_entry(BufferBinding::as_uniform(cam_id))
                .with_entry(BufferBinding::as_uniform(model_mat))
                .with_label("cube_color_bg")
        );

        let shader = ShaderSource::Embedded(include_str!(".././shaders/colored_3d.wgsl"));
        let depth_state = graphics.canvas.depth_state(
            StandardDepth, 
            wgpu::CompareFunction::Less, 
            true
        );

        let pipeline = graphics.context.request_pipeline(
            PipelineBlueprint::as_render(shader)
                .with_label("cube_colored_pip")
                .with_bg_layouts(&[bg.layout_id])
                .with_vertex_layout::<ColorVertex>()
                .with_depth_state(depth_state)
                .with_back_culling()
        );

        Self {
            vt_buffer,
            idx_buffer,
            bg,
            pipeline,
        }
    }

    pub fn render(&self, draw_cmd: &mut DrawCommand) {
        let colored_draw = IndexedDraw::new(self.pipeline, 0..36)
            .with_vertex_buffers(&[self.vt_buffer])
            .with_bind_groups(&[self.bg.id])
            .with_index_buffer(self.idx_buffer, wgpu::IndexFormat::Uint32);

        draw_cmd.add_draw(colored_draw);
    }
}

pub struct TexturedCube {
    vt_buffer: BufferId,
    idx_buffer: BufferId,

    pipeline: PipelineId,
    bg: BindGroupIdPair,
}

impl TexturedCube {
    pub fn new(graphics: &mut Graphics, cam_id: BufferId, model_mat: BufferId) -> Self {
        let (vertices, indices) = create_textured_vertices();
        let vt_buffer = graphics.context.request_buffer(
            BufferBlueprint::as_vertex().with_label("cube_colored_vertices"), 
            BufferSource::from_vec(vertices)
        );

        let idx_buffer = graphics.context.request_buffer(
            BufferBlueprint::as_index().with_label("cube_colored_indices"),
            BufferSource::from_vec(indices)
        );

        let tex = graphics.context.request_texture(
            TextureBlueprint::new(wgpu::TextureFormat::Rgba8Unorm)
                .with_label("proc_texture")
                .writable(),
            Procedural { 
                // grey/magenta checkboard
                data: vec![
                    127, 127, 127, 255,
                    255, 255, 0, 255,
                    255, 255, 0, 255,
                    127, 127, 127, 255,
                ], 
                dim: TexDimensions::size_2d(2, 2)
            }
        );

        let samp = graphics.context.request_sampler(
            Sampler::nearest().with_label("cube_sampler")
        );

        let bg = graphics.context.request_bind_group(
            BindGroupBlueprint::new()
                .with_label("cube_textured_bg")
                .with_entry(BufferBinding::as_uniform(cam_id))
                .with_entry(BufferBinding::as_uniform(model_mat))
                .with_entry(TextureBinding::as_sampled(tex, SampledTextureOptions::nonfilterable()))
                .with_entry(SamplerBinding::non_filtering(samp))
        );

        let shader = ShaderSource::Embedded(include_str!(".././shaders/textured_3d.wgsl"));
        let depth_state = graphics.canvas.depth_state(
            StandardDepth, 
            wgpu::CompareFunction::Less, 
            true
        );

        let pipeline = graphics.context.request_pipeline(
            PipelineBlueprint::as_render(shader)
                .with_label("cube_textured_pip")
                .with_bg_layouts(&[bg.layout_id])
                .with_vertex_layout::<TextureVertex>()
                .with_depth_state(depth_state)
                .with_back_culling()
        );

        Self {
            vt_buffer,
            idx_buffer,
            bg,
            pipeline,
        }
    }

    pub fn render(&self, draw_cmd: &mut DrawCommand) {
        let textured_draw = IndexedDraw::new(self.pipeline, 0..36)
            .with_vertex_buffers(&[self.vt_buffer])
            .with_index_buffer(self.idx_buffer, wgpu::IndexFormat::Uint32)
            .with_bind_groups(&[self.bg.id]);

        draw_cmd.add_draw(textured_draw);
    }
}

fn create_colored_vertices() -> (Vec<ColorVertex>, Vec<u32>) {
    let vertices = vec![
        ColorVertex { position: [ 0.5,  0.5,  0.5], color: [1.0, 0.0, 0.0]},
        ColorVertex { position: [ 0.5,  0.5, -0.5], color: [0.0, 1.0, 0.0]},
        ColorVertex { position: [ 0.5, -0.5,  0.5], color: [0.0, 0.0, 1.0]},
        ColorVertex { position: [ 0.5, -0.5, -0.5], color: [1.0, 0.0, 0.0]},
        ColorVertex { position: [-0.5,  0.5,  0.5], color: [0.0, 1.0, 0.0]},
        ColorVertex { position: [-0.5,  0.5, -0.5], color: [0.0, 0.0, 1.0]},
        ColorVertex { position: [-0.5, -0.5,  0.5], color: [1.0, 0.0, 0.0]},
        ColorVertex { position: [-0.5, -0.5, -0.5], color: [0.0, 1.0, 0.0]},
    ];

    let indices = vec![
        4, 6, 2, 4, 2, 0,   // Front
        1, 3, 7, 1, 7, 5,   // Back
        0, 2, 3, 0, 3, 1,   // Right
        5, 7, 6, 5, 6, 4,   // Left
        4, 0, 1, 4, 1, 5,   // Top
        6, 7, 3, 6, 3, 2,   // Bottom
    ];

    (vertices, indices)
}

fn create_textured_vertices() -> (Vec<TextureVertex>, Vec<u32>) {
    let vertices = vec![
        // Front
        TextureVertex { position: [-0.5,  0.5,  0.5], uv: [0.0, 1.0] },
        TextureVertex { position: [-0.5, -0.5,  0.5], uv: [0.0, 0.0] },
        TextureVertex { position: [ 0.5, -0.5,  0.5], uv: [1.0, 0.0] },
        TextureVertex { position: [ 0.5,  0.5,  0.5], uv: [1.0, 1.0] },
        // Back
        TextureVertex { position: [ 0.5,  0.5, -0.5], uv: [0.0, 1.0] },
        TextureVertex { position: [ 0.5, -0.5, -0.5], uv: [0.0, 0.0] },
        TextureVertex { position: [-0.5, -0.5, -0.5], uv: [1.0, 0.0] },
        TextureVertex { position: [-0.5,  0.5, -0.5], uv: [1.0, 1.0] },
        // Right
        TextureVertex { position: [ 0.5,  0.5,  0.5], uv: [0.0, 1.0] },
        TextureVertex { position: [ 0.5, -0.5,  0.5], uv: [0.0, 0.0] },
        TextureVertex { position: [ 0.5, -0.5, -0.5], uv: [1.0, 0.0] },
        TextureVertex { position: [ 0.5,  0.5, -0.5], uv: [1.0, 1.0] },
        // Left
        TextureVertex { position: [-0.5,  0.5, -0.5], uv: [0.0, 1.0] },
        TextureVertex { position: [-0.5, -0.5, -0.5], uv: [0.0, 0.0] },
        TextureVertex { position: [-0.5, -0.5,  0.5], uv: [1.0, 0.0] },
        TextureVertex { position: [-0.5,  0.5,  0.5], uv: [1.0, 1.0] },
        // Top
        TextureVertex { position: [-0.5,  0.5, -0.5], uv: [0.0, 1.0] },
        TextureVertex { position: [-0.5,  0.5,  0.5], uv: [0.0, 0.0] },
        TextureVertex { position: [ 0.5,  0.5,  0.5], uv: [1.0, 0.0] },
        TextureVertex { position: [ 0.5,  0.5, -0.5], uv: [1.0, 1.0] },
        // Bottom
        TextureVertex { position: [-0.5, -0.5,  0.5], uv: [0.0, 1.0] },
        TextureVertex { position: [-0.5, -0.5, -0.5], uv: [0.0, 0.0] },
        TextureVertex { position: [ 0.5, -0.5, -0.5], uv: [1.0, 0.0] },
        TextureVertex { position: [ 0.5, -0.5,  0.5], uv: [1.0, 1.0] },
    ];

    let indices = vec![
        0,  1,  2,  0,  2,  3,      // Front
        4,  5,  6,  4,  6,  7,      // Back
        8,  9,  10, 8,  10, 11,     // Right
        12, 13, 14, 12, 14, 15,     // Left
        16, 17, 18, 16, 18, 19,     // Top
        20, 21, 22, 20, 22, 23,     // Bottom
    ];

    (vertices, indices)
}
