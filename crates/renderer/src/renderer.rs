use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

use crate::camera::{Camera, CameraUniform};
use crate::overlay::{OverlayConfig, OverlayRenderer};
use crate::pipeline;
use crate::tonemap::{Tonemap, HDR_FORMAT};
use crate::vertex::{QuadVertex, StarInstance};

/// Background "colour of empty sky" in linear-light units, written as the
/// HDR pass's clear value before any stars or overlays are drawn.
///
/// In linear light a 1 cd/m² target is "1.0" by convention; the night-sky
/// background here is set to ~10⁻² cd/m² which is roughly where airglow
/// and faint integrated starlight sit at a dark site (Leinert et al. 1998
/// puts the dark-sky zenith near 22 mag/arcsec², equivalent to a few times
/// 10⁻⁴ cd/m²; the value chosen here is brighter so the sky has some
/// visible "body" through the Reinhard tone curve until the physical
/// Leinert background pass lands — see ROADMAP Phase 2.5).
const HDR_CLEAR_COLOR: wgpu::Color = wgpu::Color {
    r: 0.010,
    g: 0.010,
    b: 0.020,
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
    overlay: OverlayRenderer,
    tonemap: Tonemap,
}

impl Renderer {
    /// Build the renderer.
    ///
    /// `final_format` is the format of the *swapchain / output* the host
    /// will eventually present (typically an sRGB UNORM). The scene is
    /// rendered into a private HDR texture (`Rgba16Float`) and resolved to
    /// `final_format` by the tonemap pass on every `render` call. The host
    /// must call [`Renderer::resize`] whenever the output framebuffer
    /// changes size so the HDR target stays matched.
    pub fn new(
        device: &wgpu::Device,
        final_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        stars: &[StarInstance],
    ) -> Self {
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

        // The star and overlay passes both render to the HDR scene buffer,
        // so their pipelines are built against `HDR_FORMAT`, not the host's
        // final swapchain format.
        let pipeline = pipeline::create_pipeline(device, HDR_FORMAT, &camera_bind_group_layout);
        let overlay = OverlayRenderer::new(device, HDR_FORMAT);
        let tonemap = Tonemap::new(device, final_format, width, height);

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            camera_buffer,
            camera_bind_group,
            num_stars: stars.len() as u32,
            overlay,
            tonemap,
        }
    }

    /// Rebuild the overlay layers from `config`. Pass `OverlayConfig { layers: vec![], ..}`
    /// (or simply don't call this) to draw stars only.
    pub fn set_overlays(&mut self, device: &wgpu::Device, config: &OverlayConfig) {
        self.overlay.set_config(device, config);
    }

    /// Resize the internal HDR scene texture to match a new framebuffer
    /// size. Hosts must call this whenever their swapchain / output
    /// texture changes size; cheap no-op when the size is unchanged.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.tonemap.resize(device, width, height);
    }

    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &Camera, width: u32, height: u32) {
        let uniform = camera.uniform(width, height);
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        self.overlay.update_camera(queue, camera);
    }

    /// Render one frame.
    ///
    /// Two passes:
    ///   1. **Scene → HDR** — stars (additive) and overlays (premultiplied
    ///      additive) accumulate into a private `Rgba16Float` texture, so
    ///      faint contributions are preserved past the [0, 1] range a
    ///      UNORM target would clip them to.
    ///   2. **Tonemap → final** — a fullscreen pass samples the HDR
    ///      texture and applies a luminance-preserving Reinhard operator,
    ///      writing the result to `view` (the host's swapchain texture).
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene HDR Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.tonemap.scene_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(HDR_CLEAR_COLOR),
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

            // Overlays draw on top of stars in the same HDR pass — they
            // tonemap together with the scene, which keeps their
            // brightness perceptually consistent with the stars they
            // overlay.
            self.overlay.draw(&mut pass);
        }

        self.tonemap.draw(encoder, view);
    }
}
