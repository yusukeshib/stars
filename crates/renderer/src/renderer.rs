use std::cell::RefCell;

use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

use crate::camera::{Camera, CameraUniform, PlanetUniforms};
use crate::overlay::{OverlayConfig, OverlayRenderer};
use crate::pipeline;
use crate::skyglow::Skyglow;
use crate::text::TextRenderer;
use crate::tonemap::{Tonemap, HDR_FORMAT};
use crate::vertex::{QuadVertex, StarInstance};

/// Planet positions move slowly on naked-eye render scales, while VSOP87
/// evaluation is expensive enough to dominate per-frame CPU time. Refresh the
/// renderer-facing planet uniforms once per simulated hour (or immediately
/// when observer/refraction/enable state changes) and reuse them while the
/// camera is dragged or the realtime clock advances within that bucket.
const PLANET_CACHE_STEP_DAYS: f64 = 1.0 / 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlanetCacheKey {
    enabled: bool,
    jd_tdb_bucket: i64,
    jd_ut1_bucket: i64,
    latitude_bits: u64,
    longitude_bits: u64,
    pressure_bits: u32,
    temperature_bits: u32,
    refract: bool,
}

impl PlanetCacheKey {
    fn from_camera(camera: &Camera) -> Self {
        let bucket = |jd: f64| (jd / PLANET_CACHE_STEP_DAYS).floor() as i64;
        Self {
            enabled: camera.planets_enabled,
            jd_tdb_bucket: bucket(camera.observer.time.jd_tdb),
            jd_ut1_bucket: bucket(camera.observer.time.jd_ut1),
            latitude_bits: camera.observer.latitude_rad.to_bits(),
            longitude_bits: camera.observer.longitude_rad.to_bits(),
            pressure_bits: camera.atmosphere.pressure_hpa.to_bits(),
            temperature_bits: camera.atmosphere.temperature_c.to_bits(),
            refract: camera.atmosphere.sunlit_scattering,
        }
    }
}

struct PlanetUniformCache {
    key: Option<PlanetCacheKey>,
    uniforms: PlanetUniforms,
}

impl Default for PlanetUniformCache {
    fn default() -> Self {
        Self {
            key: None,
            uniforms: PlanetUniforms::disabled(),
        }
    }
}

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
    text: TextRenderer,
    tonemap: Tonemap,
    planet_uniform_cache: RefCell<PlanetUniformCache>,
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

        // The skyglow and star passes render to the HDR scene buffer,
        // so their pipelines are built against `HDR_FORMAT`. Overlays
        // are *UI*, not physical light — they bypass the HDR scene + the
        // adaptive tonemap and composite directly onto the post-tonemap
        // swapchain so their opacity slider behaves as users expect
        // (alpha blending in LDR space, not radiance accumulation in
        // HDR that the scene's auto-exposure subsequently squashes).
        let pipeline = pipeline::create_pipeline(device, HDR_FORMAT, &camera_bind_group_layout);
        let skyglow = Skyglow::new(device, &camera_bind_group_layout);
        let overlay = OverlayRenderer::new(device, final_format);
        let text = TextRenderer::new(device, final_format);
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
            text,
            tonemap,
            planet_uniform_cache: RefCell::new(PlanetUniformCache::default()),
        }
    }

    /// Enable or disable the skyglow/background pass. When disabled the HDR
    /// scene buffer is cleared to black and the star + overlay passes
    /// composite directly on top — useful for debugging. Earth-centred views
    /// use this pass for diffuse sky brightness; the external galactic
    /// viewpoint uses it for the top-down Milky Way disc context.
    pub fn set_skyglow_enabled(&mut self, enabled: bool) {
        self.skyglow_enabled = enabled;
    }

    /// Rebuild the overlay layers from `config`. Pass `OverlayConfig { layers: vec![], ..}`
    /// (or simply don't call this) to draw stars only.
    pub fn set_overlays(&mut self, device: &wgpu::Device, config: &OverlayConfig) {
        self.overlay.set_config(device, config);
        self.text.set_config(config);
    }

    /// Resize the internal HDR scene texture to match a new framebuffer
    /// size. Hosts must call this whenever their swapchain / output
    /// texture changes size; cheap no-op when the size is unchanged.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.tonemap
            .resize(device, &self.camera_buffer, width, height);
    }

    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &Camera, width: u32, height: u32) {
        let planet_key = PlanetCacheKey::from_camera(camera);
        let planet_uniforms = {
            let mut cache = self.planet_uniform_cache.borrow_mut();
            if cache.key != Some(planet_key) {
                cache.uniforms = camera.planet_uniforms();
                cache.key = Some(planet_key);
            }
            cache.uniforms
        };
        let uniform = camera.uniform_with_planets(width, height, &planet_uniforms);
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
        self.overlay.update_camera(queue, camera);
        self.text
            .update_camera(queue, camera, &uniform, &planet_uniforms, width, height);
    }

    /// Render one frame.
    ///
    /// Pass order:
    ///   1. **Skyglow → HDR** — the Leinert 1998 integrated-starlight
    ///      (+ DGL) surface-brightness model, attenuated by atmospheric
    ///      extinction. This is the Milky Way *band* itself.
    ///   2. **Stars → HDR** (additive) — star point sources load the
    ///      skyglow background and add their contributions. Star peaks
    ///      routinely run > 1.0 in HDR units.
    ///   3. **Tonemap → swapchain** — reads the HDR scene + the 1×1
    ///      adaptation luminance, picks a CIE-191:2010 mesopic key,
    ///      applies the Reinhard 2002 §3.3 keyed operator. Writes the
    ///      LDR result to `view`.
    ///   4. **Overlays → swapchain** — LDR alpha-blended UI lines and text
    ///      labels (horizon, cardinals, grids, ecliptic, names, ...) drawn on top of
    ///      the tonemapped scene. Bypassing the HDR/tonemap chain is
    ///      what makes the `--overlay-opacity` slider behave intuitively
    ///      — overlays are UI, not physical radiance, and shouldn't
    ///      auto-expose with the sky.
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

        // Pass 2: stars load the skyglow background and add their HDR flux.
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

            if self.num_stars > 0 {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
                pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..QuadVertex::INDICES.len() as u32, 0, 0..self.num_stars);
            }
        }

        // Pass 3: tonemap the assembled HDR scene to the swapchain.
        self.tonemap.draw(encoder, view);

        // Pass 4: overlays and labels composite onto the already-tonemapped output.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Overlay LDR Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Load whatever the tonemap pass just wrote and
                        // alpha-blend the UI lines on top.
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
            self.overlay.draw(&mut pass);
            self.text.draw(&mut pass);
        }
    }
}
