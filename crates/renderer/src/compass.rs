use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::vertex::QuadVertex;

/// Visual layout of the compass strip, in baseline pixels for a 720-tall viewport.
/// Sizes scale linearly with the actual viewport height so the strip stays the
/// same fraction of the screen at any resolution / DPR.
const BASELINE_HEIGHT: f32 = 720.0;
const STRIP_BOTTOM_BASE: f32 = 14.0;
const STRIP_HEIGHT_BASE: f32 = 36.0;
const LABEL_TOP_BASE: f32 = 0.0;
const LABEL_BASE_SCALE: f32 = 3.0; // glyph pixel = this many baseline pixels

/// 5x6 bitmap font for cardinal labels. Each row uses the low 5 bits, MSB = leftmost.
const FONT_N: [u8; 6] = [0b10001, 0b11001, 0b10101, 0b10101, 0b10011, 0b10001];
const FONT_E: [u8; 6] = [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111];
const FONT_S: [u8; 6] = [0b01111, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110];
const FONT_W: [u8; 6] = [0b10001, 0b10001, 0b10001, 0b10101, 0b11011, 0b10001];

const fn glyph_bits(rows: [u8; 6]) -> u32 {
    let mut bits = 0u32;
    let mut y = 0;
    while y < 6 {
        let row = rows[y];
        let mut x = 0;
        while x < 5 {
            // MSB-first: column 0 is bit 4 of the row byte.
            if (row >> (4 - x)) & 1 == 1 {
                bits |= 1u32 << (y * 5 + x);
            }
            x += 1;
        }
        y += 1;
    }
    bits
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct CompassUniform {
    viewport_size: [f32; 2],
    center_az_rad: f32,
    fov_x_rad: f32,
    strip_bottom_px: f32,
    strip_height_px: f32,
    label_top_px: f32,
    label_scale: f32,
    ui_scale: f32,
    // Pad so `glyphs` lands on a 16-byte boundary (WGSL vec4<u32> alignment).
    _pad: [f32; 3],
    glyphs: [u32; 4],
}

pub struct CompassRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl CompassRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Compass Vertex Buffer"),
            contents: bytemuck::cast_slice(QuadVertex::VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Compass Index Buffer"),
            contents: bytemuck::cast_slice(QuadVertex::INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Compass Uniform Buffer"),
            contents: bytemuck::bytes_of(&CompassUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compass Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(
                        std::num::NonZeroU64::new(std::mem::size_of::<CompassUniform>() as u64)
                            .unwrap(),
                    ),
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compass Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compass Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compass.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compass Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Compass Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[QuadVertex::layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        // Premultiplied-alpha "over" blending matches the shader's output
                        // (it writes vec4(a, a, a, a)).
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            bind_group,
        }
    }

    pub fn update(
        &self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        center_az_rad: f32,
        fov_x_rad: f32,
    ) {
        // Scale every dimension linearly with viewport height so the gauge has
        // the same on-screen footprint regardless of DPR or window size.
        let scale = (height as f32 / BASELINE_HEIGHT).max(0.5);
        let uniform = CompassUniform {
            viewport_size: [width as f32, height as f32],
            center_az_rad,
            fov_x_rad,
            strip_bottom_px: STRIP_BOTTOM_BASE * scale,
            strip_height_px: STRIP_HEIGHT_BASE * scale,
            label_top_px: LABEL_TOP_BASE * scale,
            label_scale: LABEL_BASE_SCALE * scale,
            ui_scale: scale,
            _pad: [0.0; 3],
            glyphs: [
                glyph_bits(FONT_N),
                glyph_bits(FONT_E),
                glyph_bits(FONT_S),
                glyph_bits(FONT_W),
            ],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Compass Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..QuadVertex::INDICES.len() as u32, 0, 0..1);
    }
}
