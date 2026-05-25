use std::path::PathBuf;

use anyhow::{Context, Result};
use astronomy::Observer;
use clap::Parser;
use renderer::{
    Atmosphere, Camera, LocalView, OverlayConfig, Renderer, StarInstance,
    DEFAULT_SCREEN_LIMITING_MAGNITUDE,
};
use stars_host_common::{
    atmosphere_from_args, load_star_instances_from_file, overlay_config_from_args,
    parse_time_to_time_scales, AtmosphereOverrides, AtmospherePresetArg, OverlayArg, ProjectionArg,
};

/// Render the night sky as seen from a given observer to a PNG.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Observer latitude in decimal degrees (north positive).
    #[arg(long, allow_hyphen_values = true)]
    lat: f64,

    /// Observer longitude in decimal degrees (east positive).
    #[arg(long, allow_hyphen_values = true)]
    lng: f64,

    /// Time as RFC3339 (e.g. 2026-04-26T12:00:00Z). Defaults to "now".
    #[arg(long)]
    time: Option<String>,

    /// Azimuth of the camera, degrees from North toward East.
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    azimuth: f64,

    /// Altitude of the camera, degrees above the horizon.
    #[arg(long, default_value_t = 30.0, allow_hyphen_values = true)]
    altitude: f64,

    /// Vertical field of view, degrees. Ignored by all-sky projections.
    #[arg(long, default_value_t = 60.0)]
    fov: f64,

    /// Screen projection: perspective, mollweide, aitoff, or hammer.
    #[arg(long, default_value_t = ProjectionArg::Perspective)]
    projection: ProjectionArg,

    /// Output image width.
    #[arg(long, default_value_t = 1280)]
    width: u32,

    /// Output image height.
    #[arg(long, default_value_t = 720)]
    height: u32,

    /// Output PNG path.
    #[arg(short, long, default_value = "stars.png")]
    output: PathBuf,

    /// Path to the HYG-format star catalog CSV.
    #[arg(long, default_value = "crates/catalog/data/hyg_v42.csv")]
    catalog: PathBuf,

    /// Overlay layers to draw. Comma-separated list, or pass --no-overlays to disable all.
    ///
    /// Possible values: horizon, cardinals, alt-az-grid, equatorial-grid,
    /// ecliptic, celestial-equator, meridian, galactic-equator,
    /// constellation-lines, constellation-boundaries, star-labels,
    /// planet-labels, constellation-labels, cardinal-labels, degree-labels.
    #[arg(
        long,
        value_delimiter = ',',
        default_values_t = vec![OverlayArg::Horizon, OverlayArg::Cardinals, OverlayArg::CardinalLabels],
    )]
    overlays: Vec<OverlayArg>,

    /// Disable all overlays (overrides --overlays).
    #[arg(long)]
    no_overlays: bool,

    /// Spacing between alt-az / RA-Dec grid lines, in degrees.
    #[arg(long, default_value_t = 15.0)]
    grid_step_deg: f64,

    /// Opacity of overlay lines (0..=1). Text labels remain fully opaque.
    #[arg(long, default_value_t = 0.6)]
    overlay_opacity: f32,

    /// Limiting apparent magnitude of the simulated observer. Stars fainter
    /// than this fade through the shader's discard cutoff. Increasing this
    /// uniformly scales every star's linear flux ("more sensitive observer"
    /// / "longer exposure") without breaking Pogson's law.
    ///
    /// 6.0 = strict dark-adapted naked eye; 7.5 ≈ binocular visual limit and
    /// is a good default for indoor screens.
    #[arg(long, default_value_t = DEFAULT_SCREEN_LIMITING_MAGNITUDE)]
    limiting_magnitude: f32,

    /// Disable atmospheric extinction and sunlit sky scattering. With the
    /// default atmosphere on, stars near the horizon dim/redden and daylight
    /// or twilight sky colour is driven by the Sun position. This flag turns
    /// the atmosphere off, so every star renders at catalogue magnitude and
    /// the sky background contains only non-atmospheric components.
    #[arg(long)]
    no_extinction: bool,

    /// Atmosphere preset used as the base for extinction and sky colour.
    #[arg(long, default_value_t = AtmospherePresetArg::ClearRural)]
    atmosphere_preset: AtmospherePresetArg,

    /// Override aerosol / haze turbidity for sunlit sky scattering. Around
    /// 2–3 is a clear rural sky; larger values whiten and brighten the horizon.
    #[arg(long)]
    turbidity: Option<f32>,

    /// Override observer altitude above sea level in metres for the sunlit
    /// scattering optical-depth approximation.
    #[arg(long)]
    observer_altitude_m: Option<f32>,

    /// Override total ozone column in Dobson units for sunset/twilight colour.
    #[arg(long)]
    ozone_du: Option<f32>,

    /// Override meteorological visibility in kilometres for aerosol haze.
    #[arg(long)]
    visibility_km: Option<f32>,

    /// Override surface pressure in hPa for atmospheric refraction.
    #[arg(long)]
    pressure_hpa: Option<f32>,

    /// Override air temperature in °C for atmospheric refraction.
    #[arg(long, allow_hyphen_values = true)]
    temperature_c: Option<f32>,

    /// Disable Mercury-through-Neptune rendering.
    #[arg(long)]
    no_planets: bool,

    /// Disable the diffuse-sky (integrated starlight + diffuse galactic
    /// light) skyglow pass. With the default (skyglow on), the sky
    /// background includes the analytic Leinert et al. 1998 model so the
    /// Milky Way band is visible against the dark sky.
    #[arg(long)]
    no_skyglow: bool,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    let time = parse_time_to_time_scales(args.time.as_deref())?;
    log::info!(
        "Observer lat={} lng={} jd_utc={} jd_ut1={} jd_tdb={}",
        args.lat,
        args.lng,
        time.jd_utc,
        time.jd_ut1,
        time.jd_tdb
    );

    let observer = Observer::from_degrees_with_time(args.lat, args.lng, time);
    let view = LocalView {
        azimuth_rad: (args.azimuth as f32).to_radians(),
        altitude_rad: (args.altitude as f32).to_radians(),
        fov_y_rad: (args.fov as f32).to_radians(),
    };

    let limiting_mag = args.limiting_magnitude;
    let instances = load_star_instances_from_file(&args.catalog, limiting_mag)?;
    log::info!("Loaded {} stars", instances.len());

    let overlays = overlay_config_from_args(
        args.no_overlays,
        &args.overlays,
        args.grid_step_deg,
        args.overlay_opacity,
    );
    let atmosphere = atmosphere_from_args(
        args.no_extinction,
        args.atmosphere_preset,
        AtmosphereOverrides {
            turbidity: args.turbidity,
            observer_altitude_m: args.observer_altitude_m,
            ozone_du: args.ozone_du,
            visibility_km: args.visibility_km,
            pressure_hpa: args.pressure_hpa,
            temperature_c: args.temperature_c,
        },
    );
    let skyglow_enabled = !args.no_skyglow;

    let pixels = pollster::block_on(render_to_pixels(
        observer,
        view,
        atmosphere,
        skyglow_enabled,
        !args.no_planets,
        args.projection.into(),
        limiting_mag,
        args.width,
        args.height,
        &instances,
        &overlays,
    ))?;

    let img = image::RgbaImage::from_raw(args.width, args.height, pixels)
        .context("Pixel buffer size mismatch")?;
    img.save(&args.output)
        .with_context(|| format!("Failed to write {}", args.output.display()))?;

    log::info!("Wrote {}", args.output.display());
    Ok(())
}

const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

#[allow(clippy::too_many_arguments)]
async fn render_to_pixels(
    observer: Observer,
    view: LocalView,
    atmosphere: Atmosphere,
    skyglow_enabled: bool,
    planets_enabled: bool,
    projection: renderer::SkyProjection,
    limiting_mag: f32,
    width: u32,
    height: u32,
    stars: &[StarInstance],
    overlays: &OverlayConfig,
) -> Result<Vec<u8>> {
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
            width,
            height,
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
    let unpadded_bytes_per_row = width * bytes_per_pixel;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Readback Buffer"),
        size: (padded_bytes_per_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut renderer = Renderer::new(&device, TEXTURE_FORMAT, width, height, stars);
    renderer.set_overlays(&device, overlays);
    renderer.set_skyglow_enabled(skyglow_enabled);
    let mut camera = Camera::new(observer, view, width as f32 / height as f32);
    camera.atmosphere = atmosphere;
    camera.planets_enabled = planets_enabled;
    camera.projection = projection;
    camera.limiting_magnitude = limiting_mag;
    renderer.update_camera(&queue, &camera, width, height);

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
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
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
    let mut pixels = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
    for row in 0..height {
        let start = (row * padded_bytes_per_row) as usize;
        let end = start + unpadded_bytes_per_row as usize;
        pixels.extend_from_slice(&data[start..end]);
    }
    drop(data);
    output_buffer.unmap();

    Ok(pixels)
}
