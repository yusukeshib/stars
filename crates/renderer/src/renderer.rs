use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

use crate::camera::{Camera, CameraUniform};
use crate::overlay::{OverlayConfig, OverlayRenderer};
use crate::pipeline;
use crate::skyglow::Skyglow;
use crate::tonemap::{Tonemap, HDR_FORMAT};
use crate::vertex::{QuadVertex, StarInstance};

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    num_stars: u32,
    skyglow: Skyglow,
    skyglow_enabled: bool,
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

        // The skyglow, star and overlay passes all render to the HDR scene
        // buffer, so their pipelines are built against `HDR_FORMAT`, not
        // the host's final swapchain format.
        let pipeline = pipeline::create_pipeline(device, HDR_FORMAT, &camera_bind_group_layout);
        let skyglow = Skyglow::new(device, &camera_bind_group_layout);
        let overlay = OverlayRenderer::new(device, HDR_FORMAT);
        // Tonemap pass borrows the camera buffer directly (it samples
        // `magnitude_zeropoint` for the HDR-flux→cd/m² conversion that
        // drives the mesopic regime split).
        let tonemap = Tonemap::new(device, final_format, &camera_buffer, width, height);

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            camera_buffer,
            camera_bind_group,
            num_stars: stars.len() as u32,
            skyglow,
            skyglow_enabled: true,
            overlay,
            tonemap,
        }
    }

    /// Enable or disable the diffuse skyglow pass. When disabled the HDR
    /// scene buffer is cleared to black and the star + overlay passes
    /// composite directly on top — useful for debugging or for views from
    /// outside the Earth-atmosphere context (where the Milky Way's
    /// integrated starlight model also stops making physical sense).
    pub fn set_skyglow_enabled(&mut self, enabled: bool) {
        self.skyglow_enabled = enabled;
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
        self.tonemap
            .resize(device, &self.camera_buffer, width, height);
    }

    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &Camera, width: u32, height: u32) {
        let uniform = camera.uniform(width, height);
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        self.overlay.update_camera(queue, camera);
    }

    /// Render one frame.
    ///
    /// Pass order:
    ///   1. **Skyglow → HDR** — a fullscreen pass writes the Leinert
    ///      1998 integrated-starlight (+ DGL) surface-brightness model
    ///      into the HDR scene buffer, attenuated by atmospheric
    ///      extinction. This is the Milky Way *band* itself.
    ///   2. **Stars + overlays → HDR** (additive) — stars and overlay
    ///      lines load the skyglow background and add their contributions
    ///      on top. Star peaks routinely run > 1.0 in HDR units.
    ///   3. **Tonemap → final** — a fullscreen pass samples the HDR
    ///      texture and applies a luminance-preserving Reinhard operator,
    ///      writing the result to `view` (the host's swapchain texture).
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        // Pass 1: skyglow fills the HDR buffer with the diffuse-sky
        // baseline (clears to black, then writes the Leinert ISL+DGL
        // model evaluated per fragment). When disabled the clear still
        // happens — we just skip the draw call so the buffer stays black
        // for the star + overlay passes.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Skyglow HDR Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.tonemap.scene_view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if self.skyglow_enabled {
                self.skyglow.draw(&mut pass, &self.camera_bind_group);
            }
        }

        // Pass 2: stars + overlays load the skyglow background and add.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Scene HDR Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.tonemap.scene_view(),
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

        // Pass 3: tonemap the assembled HDR scene to the swapchain.
        self.tonemap.draw(encoder, view);
    }
}
