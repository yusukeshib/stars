//! Shared headless GPU render pipeline used by every native host.
//!
//! Lives in `crates/common` (host glue, per `ARCHITECTURE.md`) so the CLI
//! (`apps/cli`) and the HTTP server (`apps/server`, V-L-22) drive a single
//! GPU + scene → pixels routine. The engine crate `renderer` stays free of
//! wgpu device/adapter ownership and image encoding — those are host-tier
//! concerns and belong here.

use std::path::Path;

use anyhow::{Context, Result};
use astronomy::Observer;
use renderer::{Camera, Renderer, StarInstance};

use crate::{load_star_instances_from_file_at, SessionScene};

/// Render target format used by every native host. `Rgba8UnormSrgb` matches
/// the CLI's previous local constant and the swap-chain format the desktop
/// viewer uses, so the GPU tone-mapping path is identical across hosts.
pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Output-image control inputs that aren't part of the persisted scene JSON.
/// Width / height come from the host (CLI flag, HTTP query) rather than the
/// session so a single saved scene can be re-rendered at any resolution.
#[derive(Debug, Clone, Copy)]
pub struct RenderOptions {
    pub width: u32,
    pub height: u32,
    /// Mirrors the CLI's `--no-skyglow` flag. The session itself doesn't
    /// pin this because hosts may want to A/B with and without the diffuse
    /// pass for the same scene.
    pub skyglow_enabled: bool,
    /// `L-20`: render known variable stars at their phase-folded magnitude for
    /// `scene.time`. Off by default (catalogue purity) so existing headless /
    /// preset renders stay byte-identical; the CLI exposes `--variable-magnitudes`.
    pub variable_magnitudes: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            skyglow_enabled: true,
            variable_magnitudes: false,
        }
    }
}

/// Render `scene` to an RGBA8 pixel buffer of size `options.width *
/// options.height * 4`.
///
/// `stars` is the catalog-derived instance buffer. Callers that load from
/// the on-disk HYG CSV should use [`render_scene_from_catalog_path`], which
/// resolves the catalog path off `scene.catalog` (falling back to the
/// supplied default) just like the CLI used to do inline.
#[allow(clippy::too_many_arguments)]
pub async fn render_scene_pixels(
    scene: &SessionScene,
    stars: &[StarInstance],
    options: RenderOptions,
) -> Result<Vec<u8>> {
    let observer =
        Observer::from_degrees_with_time(scene.latitude_deg, scene.longitude_deg, scene.time);

    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .context("No suitable GPU adapter found")?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Stars Headless Device"),
            ..Default::default()
        })
        .await
        .context("Failed to create device")?;

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Render Target"),
        size: wgpu::Extent3d {
            width: options.width,
            height: options.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TEXTURE_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let bytes_per_pixel: u32 = 4;
    let unpadded_bytes_per_row = options.width * bytes_per_pixel;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Readback Buffer"),
        size: (padded_bytes_per_row * options.height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut renderer = Renderer::new(
        &device,
        TEXTURE_FORMAT,
        options.width,
        options.height,
        stars,
    );
    renderer.set_overlays(&device, &scene.overlays);
    renderer.set_skyglow_enabled(options.skyglow_enabled);
    let mut camera = Camera::new(
        observer,
        scene.view,
        options.width as f32 / options.height as f32,
    );
    camera.atmosphere = scene.atmosphere;
    camera.scintillation = scene.scintillation;
    camera.light_pollution = crate::resolve_light_pollution(scene.light_pollution);
    camera.planets_enabled = scene.planets_enabled;
    camera.satellites = scene.satellites.clone();
    camera.meteors = scene.meteors.clone();
    camera.comets = scene.comets.clone();
    camera.projection = scene.projection;
    camera.viewpoint = scene.viewpoint;
    camera.external_viewpoint = scene.external_viewpoint;
    camera.eyepiece = scene.eyepiece;
    camera.limiting_magnitude = scene.catalog.limiting_magnitude;
    camera.output_colourspace = scene.output_colourspace;
    camera.aurora = scene.aurora;
    renderer.update_camera(&queue, &camera, options.width, options.height);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Headless Encoder"),
    });

    renderer.render(&mut encoder, &target_view);

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &output_buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(options.height),
            },
        },
        wgpu::Extent3d {
            width: options.width,
            height: options.height,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(std::iter::once(encoder.finish()));

    let buffer_slice = output_buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .context("device.poll failed")?;
    rx.recv()
        .context("Buffer mapping channel closed")?
        .context("Buffer mapping failed")?;

    let data = buffer_slice.get_mapped_range();
    let mut pixels =
        Vec::with_capacity((options.width * options.height * bytes_per_pixel) as usize);
    for row in 0..options.height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + unpadded_bytes_per_row as usize;
        pixels.extend_from_slice(&data[start..end]);
    }
    drop(data);
    output_buffer.unmap();

    Ok(pixels)
}

/// Resolve the catalog path off `scene.catalog` (falling back to `default`),
/// load star instances at the scene's limiting magnitude, and render. This
/// is the exact path the CLI takes for a JSON session; the HTTP server
/// reuses it so the two hosts can never drift on catalog selection rules.
pub async fn render_scene_from_catalog_path(
    scene: &SessionScene,
    default_catalog: impl AsRef<Path>,
    options: RenderOptions,
) -> Result<Vec<u8>> {
    let catalog_path: std::path::PathBuf = scene
        .catalog
        .path
        .as_deref()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| default_catalog.as_ref().to_path_buf());
    let variable_jd = options.variable_magnitudes.then_some(scene.time.jd_utc);
    let instances = load_star_instances_from_file_at(
        &catalog_path,
        scene.catalog.limiting_magnitude,
        variable_jd,
    )
    .with_context(|| {
        format!(
            "Loading star catalog at {} for render",
            catalog_path.display()
        )
    })?;
    render_scene_pixels(scene, &instances, options).await
}

/// Encode a raw RGBA8 buffer as a PNG byte stream. Used by the HTTP server's
/// `/render` route, which returns the bytes directly instead of writing to
/// disk like the CLI does.
pub fn encode_png(width: u32, height: u32, pixels: Vec<u8>) -> Result<Vec<u8>> {
    let img =
        image::RgbaImage::from_raw(width, height, pixels).context("Pixel buffer size mismatch")?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .context("Encoding PNG buffer")?;
    Ok(buf.into_inner())
}
