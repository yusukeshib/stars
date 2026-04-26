use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

use crate::camera::{Camera, CameraUniform};
use crate::pipeline;
use crate::vertex::{QuadVertex, StarInstance};

/// Background color for the sky (very dark blue).
const CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.01,
    g: 0.01,
    b: 0.02,
    a: 1.0,
};

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    num_stars: u32,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, stars: &[StarInstance]) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(QuadVertex::VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Index Buffer"),
            contents: bytemuck::cast_slice(QuadVertex::INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Star Instance Buffer"),
            contents: bytemuck::cast_slice(stars),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Uniform Buffer"),
            contents: bytemuck::bytes_of(&CameraUniform::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout = pipeline::create_camera_bind_group_layout(device);

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let pipeline = pipeline::create_pipeline(device, format, &camera_bind_group_layout);

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            camera_buffer,
            camera_bind_group,
            num_stars: stars.len() as u32,
        }
    }

    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &Camera, width: u32, height: u32) {
        let uniform = camera.uniform(width, height);
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Star Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR_COLOR),
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
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..QuadVertex::INDICES.len() as u32, 0, 0..self.num_stars);
    }
}
