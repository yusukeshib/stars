//! Shared headless GPU render pipeline used by every native host.
//!
//! Lives in `crates/common` (host glue, per `ARCHITECTURE.md`) so the CLI
//! (`apps/cli`) and the HTTP server (`apps/server`, V-L-22) drive a single
//! GPU + scene → pixels routine. The engine crate `renderer` stays free of
//! wgpu device/adapter ownership and image encoding — those are host-tier
//! concerns and belong here.

use std::path::Path;

use anyhow::{bail, Context, Result};
use astronomy::Observer;
use renderer::{Camera, Renderer, StarInstance};

use crate::{load_star_instances_for_backend, verify_catalog_digest, SessionScene};
use catalog::CatalogBackendKind;

/// Render target format used by every native host. `Rgba8UnormSrgb` matches
/// the CLI's previous local constant and the swap-chain format the desktop
/// viewer uses, so the GPU tone-mapping path is identical across hosts.
pub const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Bounds shared by every headless host. The pixel budget limits aggregate GPU
/// and readback memory even when each individual dimension is within range.
pub const MIN_RENDER_DIMENSION: u32 = 16;
pub const MAX_RENDER_DIMENSION: u32 = 8192;
pub const MAX_RENDER_PIXELS: u64 = 16_777_216;
const BYTES_PER_PIXEL: u32 = 4;

#[derive(Debug, Clone, Copy)]
struct RenderLayout {
    unpadded_bytes_per_row: u32,
    padded_bytes_per_row: u32,
    readback_bytes: u64,
    pixel_bytes: usize,
}

fn render_layout(width: u32, height: u32) -> Result<RenderLayout> {
    if !(MIN_RENDER_DIMENSION..=MAX_RENDER_DIMENSION).contains(&width)
        || !(MIN_RENDER_DIMENSION..=MAX_RENDER_DIMENSION).contains(&height)
    {
        bail!(
            "render dimensions must each be in {MIN_RENDER_DIMENSION}..={MAX_RENDER_DIMENSION}; got {width}x{height}"
        );
    }

    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .context("render pixel count overflow")?;
    if pixel_count > MAX_RENDER_PIXELS {
        bail!(
            "render pixel count {pixel_count} exceeds budget {MAX_RENDER_PIXELS} ({width}x{height})"
        );
    }

    let unpadded_bytes_per_row = width
        .checked_mul(BYTES_PER_PIXEL)
        .context("render row byte count overflow")?;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_units = unpadded_bytes_per_row
        .checked_add(alignment - 1)
        .context("aligned render row byte count overflow")?
        / alignment;
    let padded_bytes_per_row = padded_units
        .checked_mul(alignment)
        .context("aligned render row byte count overflow")?;
    let readback_bytes = u64::from(padded_bytes_per_row)
        .checked_mul(u64::from(height))
        .context("render readback buffer size overflow")?;
    let pixel_bytes_u64 = pixel_count
        .checked_mul(u64::from(BYTES_PER_PIXEL))
        .context("render output byte count overflow")?;
    let pixel_bytes = usize::try_from(pixel_bytes_u64)
        .context("render output does not fit this platform's address space")?;

    Ok(RenderLayout {
        unpadded_bytes_per_row,
        padded_bytes_per_row,
        readback_bytes,
        pixel_bytes,
    })
}

/// Validate headless dimensions before catalog loading or GPU work begins.
pub fn validate_render_dimensions(width: u32, height: u32) -> Result<()> {
    render_layout(width, height).map(|_| ())
}

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
    let layout = render_layout(options.width, options.height)?;
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

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Readback Buffer"),
        size: layout.readback_bytes,
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
    renderer.set_deep_sky_markers(&crate::deep_sky_markers());
    renderer.set_sky_labels(&crate::sky_labels());
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
                bytes_per_row: Some(layout.padded_bytes_per_row),
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
    let mut pixels = Vec::with_capacity(layout.pixel_bytes);
    for row in 0..options.height {
        let start = usize::try_from(u64::from(row) * u64::from(layout.padded_bytes_per_row))
            .context("render row offset does not fit address space")?;
        let end = start + layout.unpadded_bytes_per_row as usize;
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
    verify_catalog_digest(&catalog_path, scene.catalog.hash.as_deref())?;
    let variable_jd = options.variable_magnitudes.then_some(scene.time.jd_utc);
    // L-17: re-select the session's recorded backend; unknown / legacy labels
    // fall back to HYG so older sessions still render.
    let backend = CatalogBackendKind::from_kebab_str(&scene.catalog.backend)
        .unwrap_or(CatalogBackendKind::HygCsv);
    let instances = load_star_instances_for_backend(
        backend,
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
pub fn encode_png(
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    colourspace: renderer::OutputColourSpace,
) -> Result<Vec<u8>> {
    validate_render_dimensions(width, height)?;
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .context("Pixel buffer size overflow")?;
    anyhow::ensure!(pixels.len() == expected, "Pixel buffer size mismatch");
    let mut bytes = Vec::new();
    write_png_to(&mut bytes, width, height, &pixels, colourspace)?;
    Ok(bytes)
}

pub fn write_png(
    path: &std::path::Path,
    width: u32,
    height: u32,
    pixels: &[u8],
    colourspace: renderer::OutputColourSpace,
) -> Result<()> {
    validate_render_dimensions(width, height)?;
    let file = std::fs::File::create(path)
        .with_context(|| format!("Failed to create {}", path.display()))?;
    write_png_to(
        std::io::BufWriter::new(file),
        width,
        height,
        pixels,
        colourspace,
    )
}

fn write_png_to(
    output: impl std::io::Write,
    width: u32,
    height: u32,
    pixels: &[u8],
    colourspace: renderer::OutputColourSpace,
) -> Result<()> {
    let mut encoder = png::Encoder::new(output, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    match colourspace {
        renderer::OutputColourSpace::Srgb => {
            encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        }
        renderer::OutputColourSpace::DisplayP3 | renderer::OutputColourSpace::Rec2020 => {
            // Pixels use the renderer's documented sRGB transfer curve even
            // when the primaries are P3/Rec.2020. PNG gAMA cannot encode the
            // piecewise curve exactly, but 1/2.2 is the standard interoperable
            // signal alongside cHRM when no embedded ICC profile is available.
            encoder.set_source_gamma(png::ScaledFloat::from_scaled(45_455));
            let [red, green, blue, white] = colourspace.primaries_xy();
            encoder.set_source_chromaticities(png::SourceChromaticities::new(
                (white.0, white.1),
                (red.0, red.1),
                (green.0, green.1),
                (blue.0, blue.1),
            ));
        }
    }
    let mut writer = encoder
        .write_header()
        .context("Failed to write PNG header")?;
    writer
        .write_image_data(pixels)
        .context("Failed to write PNG image data")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_dimensions_enforce_bounds_and_pixel_budget() {
        assert!(validate_render_dimensions(1280, 720).is_ok());
        assert!(validate_render_dimensions(8192, 2048).is_ok());
        assert!(validate_render_dimensions(0, 720).is_err());
        assert!(validate_render_dimensions(8193, 720).is_err());
        assert!(validate_render_dimensions(8192, 8192).is_err());
    }

    #[test]
    fn render_layout_accounts_for_row_padding_with_checked_sizes() {
        let layout = render_layout(17, 16).unwrap();
        assert_eq!(layout.unpadded_bytes_per_row, 68);
        assert_eq!(layout.padded_bytes_per_row, 256);
        assert_eq!(layout.readback_bytes, 4096);
        assert_eq!(layout.pixel_bytes, 17 * 16 * 4);
    }

    #[test]
    fn png_encoder_tags_the_requested_colourspace() {
        let pixels = vec![0_u8; 16 * 16 * 4];
        let srgb = encode_png(16, 16, pixels.clone(), renderer::OutputColourSpace::Srgb).unwrap();
        assert!(srgb.windows(4).any(|chunk| chunk == b"sRGB"));

        for colourspace in [
            renderer::OutputColourSpace::DisplayP3,
            renderer::OutputColourSpace::Rec2020,
        ] {
            let png = encode_png(16, 16, pixels.clone(), colourspace).unwrap();
            assert!(png.windows(4).any(|chunk| chunk == b"cHRM"));
            assert!(png.windows(4).any(|chunk| chunk == b"gAMA"));
            assert!(!png.windows(4).any(|chunk| chunk == b"sRGB"));
        }
    }
}
