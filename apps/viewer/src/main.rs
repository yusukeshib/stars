use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use astronomy::Observer;
use clap::Parser;
use renderer::{
    Atmosphere, Camera, LocalView, OverlayConfig, Renderer, StarInstance,
    DEFAULT_SCREEN_LIMITING_MAGNITUDE,
};
use stars_host_common::{
    atmosphere_from_args, load_star_instances_from_file, overlay_config_from_args,
    parse_time_to_time_scales, AtmosphereOverrides, AtmospherePresetArg, OverlayArg, ProjectionArg,
    ViewpointArg,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const INITIAL_WINDOW_WIDTH: u32 = 1280;
const INITIAL_WINDOW_HEIGHT: u32 = 720;
/// Convert high-resolution trackpad pixel deltas into approximately the same
/// units as line-wheel deltas before applying the zoom exponential.
const PIXEL_SCROLL_TO_LINE_DELTA: f32 = 0.01;
/// Exponential zoom sensitivity per scroll unit. A small value keeps wheel and
/// trackpad zoom smooth rather than jumping between FoV clamp limits.
const ZOOM_SCROLL_SENSITIVITY: f32 = 0.05;
const CLOCK_SPEED_REALTIME: f64 = 1.0;
const CLOCK_SPEED_MINUTE_PER_SECOND: f64 = 60.0;
const CLOCK_SPEED_HOUR_PER_SECOND: f64 = 3_600.0;
const CLOCK_SPEED_DAY_PER_SECOND: f64 = 86_400.0;

/// Interactive desktop viewer for the night sky.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Observer latitude in decimal degrees (north positive).
    #[arg(long, allow_hyphen_values = true, default_value_t = 35.68)]
    lat: f64,

    /// Observer longitude in decimal degrees (east positive).
    #[arg(long, allow_hyphen_values = true, default_value_t = 139.69)]
    lng: f64,

    /// Initial time as RFC3339. Defaults to "now". The clock advances in real time.
    #[arg(long)]
    time: Option<String>,

    /// Initial azimuth in degrees from North toward East.
    #[arg(long, default_value_t = 180.0, allow_hyphen_values = true)]
    azimuth: f64,

    /// Initial altitude in degrees above the horizon.
    #[arg(long, default_value_t = 30.0, allow_hyphen_values = true)]
    altitude: f64,

    /// Initial vertical field of view, degrees. Ignored by all-sky projections.
    #[arg(long, default_value_t = 70.0)]
    fov: f64,

    /// Screen projection: perspective, mollweide, aitoff, or hammer.
    #[arg(long, default_value_t = ProjectionArg::Perspective)]
    projection: ProjectionArg,

    /// Camera location: earth (observer-centred sky) or galactic-north
    /// (external top-down Milky Way disc map).
    #[arg(long, default_value_t = ViewpointArg::Earth)]
    viewpoint: ViewpointArg,

    /// Path to the HYG-format star catalog CSV.
    #[arg(long, default_value = "crates/catalog/data/hyg_v42.csv")]
    catalog: PathBuf,

    /// Overlay layers to draw. Comma-separated; pass --no-overlays to disable all.
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

    /// Disable atmospheric extinction and sunlit sky scattering.
    #[arg(long)]
    no_extinction: bool,

    /// Atmosphere preset used as the base for extinction and sky colour.
    #[arg(long, default_value_t = AtmospherePresetArg::ClearRural)]
    atmosphere_preset: AtmospherePresetArg,

    /// Override aerosol / haze turbidity for sunlit sky scattering.
    #[arg(long)]
    turbidity: Option<f32>,

    /// Override observer altitude above sea level in metres.
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
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    let start_jd = parse_time_to_time_scales(args.time.as_deref())?.jd_utc;
    let limiting_mag = DEFAULT_SCREEN_LIMITING_MAGNITUDE;
    let instances = load_star_instances_from_file(&args.catalog, limiting_mag)?;
    log::info!("Loaded {} stars", instances.len());

    let initial_view = LocalView {
        azimuth_rad: (args.azimuth as f32).to_radians(),
        altitude_rad: (args.altitude as f32).to_radians(),
        fov_y_rad: (args.fov as f32).to_radians(),
    };

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

    let event_loop = EventLoop::new()?;
    let mut app = App::new(
        instances,
        args.lat,
        args.lng,
        start_jd,
        initial_view,
        overlays,
        limiting_mag,
        atmosphere,
        !args.no_planets,
        args.projection.into(),
        args.viewpoint.into(),
    );
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    camera: Camera,
    size: PhysicalSize<u32>,
}

struct App {
    gpu: Option<GpuState>,
    window: Option<Arc<Window>>,
    stars: Vec<StarInstance>,
    lat: f64,
    lng: f64,
    initial_view: LocalView,
    overlays: OverlayConfig,
    limiting_magnitude: f32,
    atmosphere: Atmosphere,
    planets_enabled: bool,
    projection: renderer::SkyProjection,
    viewpoint: renderer::SkyViewpoint,
    sky_clock: SkyClock,
    mouse_pressed: bool,
    last_mouse: Option<(f64, f64)>,
}

/// Tracks the rendered moment in JD, advancing real time at a configurable speed.
struct SkyClock {
    /// JD that corresponds to `epoch`.
    anchor_jd: f64,
    /// Real-time instant the anchor was set.
    epoch: Instant,
    /// Multiplier on real-time elapsed seconds. 1.0 = real time, 60.0 = a minute per second.
    speed: f64,
    /// When paused, the JD frozen at the moment of pausing.
    paused_at: Option<f64>,
}

impl SkyClock {
    fn new(start_jd: f64) -> Self {
        Self {
            anchor_jd: start_jd,
            epoch: Instant::now(),
            speed: CLOCK_SPEED_REALTIME,
            paused_at: None,
        }
    }

    fn current_jd(&self) -> f64 {
        if let Some(jd) = self.paused_at {
            jd
        } else {
            self.anchor_jd + self.epoch.elapsed().as_secs_f64() * self.speed / 86_400.0
        }
    }

    fn set_speed(&mut self, speed: f64) {
        self.anchor_jd = self.current_jd();
        self.epoch = Instant::now();
        self.speed = speed;
    }

    fn toggle_pause(&mut self) {
        match self.paused_at {
            Some(jd) => {
                self.anchor_jd = jd;
                self.epoch = Instant::now();
                self.paused_at = None;
            }
            None => self.paused_at = Some(self.current_jd()),
        }
    }
}

impl App {
    #[allow(clippy::too_many_arguments)]
    fn new(
        stars: Vec<StarInstance>,
        lat: f64,
        lng: f64,
        start_jd: f64,
        initial_view: LocalView,
        overlays: OverlayConfig,
        limiting_magnitude: f32,
        atmosphere: Atmosphere,
        planets_enabled: bool,
        projection: renderer::SkyProjection,
        viewpoint: renderer::SkyViewpoint,
    ) -> Self {
        Self {
            gpu: None,
            window: None,
            stars,
            lat,
            lng,
            initial_view,
            overlays,
            limiting_magnitude,
            atmosphere,
            planets_enabled,
            projection,
            viewpoint,
            sky_clock: SkyClock::new(start_jd),
            mouse_pressed: false,
            last_mouse: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = Window::default_attributes()
            .with_title("Stars")
            .with_inner_size(PhysicalSize::new(
                INITIAL_WINDOW_WIDTH,
                INITIAL_WINDOW_HEIGHT,
            ));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                log::error!("failed to create viewer window: {error}");
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window.clone());

        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = match instance.create_surface(window.clone()) {
            Ok(surface) => surface,
            Err(error) => {
                log::error!("failed to create wgpu surface: {error}");
                event_loop.exit();
                return;
            }
        };
        let adapter =
            match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })) {
                Ok(adapter) => adapter,
                Err(error) => {
                    log::error!("no suitable GPU adapter found: {error}");
                    event_loop.exit();
                    return;
                }
            };

        let (device, queue) =
            match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("Stars Viewer Device"),
                ..Default::default()
            })) {
                Ok(device_and_queue) => device_and_queue,
                Err(error) => {
                    log::error!("failed to create wgpu device: {error}");
                    event_loop.exit();
                    return;
                }
            };

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let mut renderer = Renderer::new(&device, format, size.width, size.height, &self.stars);
        renderer.set_overlays(&device, &self.overlays);
        let observer = Observer::from_degrees(self.lat, self.lng, self.sky_clock.current_jd());
        let mut camera = Camera::new(
            observer,
            self.initial_view,
            size.width as f32 / size.height as f32,
        );
        camera.limiting_magnitude = self.limiting_magnitude;
        camera.atmosphere = self.atmosphere;
        camera.planets_enabled = self.planets_enabled;
        camera.projection = self.projection;
        camera.viewpoint = self.viewpoint;

        self.gpu = Some(GpuState {
            surface,
            device,
            queue,
            config,
            renderer,
            camera,
            size,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Snapshot self-state needed inside the gpu match arms before borrowing
        // self.gpu mutably (Rust can't see that lat/lng/sky_clock and gpu don't alias).
        let jd = self.sky_clock.current_jd();
        let lat = self.lat;
        let lng = self.lng;
        let Some(gpu) = &mut self.gpu else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(new_size) if new_size.width > 0 && new_size.height > 0 => {
                gpu.size = new_size;
                gpu.config.width = new_size.width;
                gpu.config.height = new_size.height;
                gpu.surface.configure(&gpu.device, &gpu.config);
                gpu.camera.aspect = new_size.width as f32 / new_size.height as f32;
                // Keep the renderer's HDR target matched to the swapchain.
                gpu.renderer
                    .resize(&gpu.device, new_size.width, new_size.height);
            }

            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                self.mouse_pressed = state == ElementState::Pressed;
                if !self.mouse_pressed {
                    self.last_mouse = None;
                }
            }

            WindowEvent::CursorMoved { position, .. } if self.mouse_pressed => {
                if let Some((lx, ly)) = self.last_mouse {
                    // Pixel drag scaled by current FOV so it feels consistent at any zoom.
                    let scale = gpu.camera.view.fov_y_rad / gpu.size.height as f32;
                    let daz = -(position.x - lx) as f32 * scale;
                    let dalt = (position.y - ly) as f32 * scale;
                    gpu.camera.rotate_view(daz, dalt);
                }
                self.last_mouse = Some((position.x, position.y));
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        p.y as f32 * PIXEL_SCROLL_TO_LINE_DELTA
                    }
                };
                gpu.camera
                    .zoom_fov((-scroll * ZOOM_SCROLL_SENSITIVITY).exp());
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(code),
                        ..
                    },
                ..
            } => match code {
                KeyCode::Space => self.sky_clock.toggle_pause(),
                KeyCode::Digit1 => self.sky_clock.set_speed(CLOCK_SPEED_REALTIME),
                KeyCode::Digit2 => self.sky_clock.set_speed(CLOCK_SPEED_MINUTE_PER_SECOND),
                KeyCode::Digit3 => self.sky_clock.set_speed(CLOCK_SPEED_HOUR_PER_SECOND),
                KeyCode::Digit4 => self.sky_clock.set_speed(CLOCK_SPEED_DAY_PER_SECOND),
                KeyCode::Escape => event_loop.exit(),
                _ => {}
            },

            WindowEvent::RedrawRequested => {
                gpu.camera.observer = Observer::from_degrees(lat, lng, jd);
                gpu.renderer.update_camera(
                    &gpu.queue,
                    &gpu.camera,
                    gpu.size.width,
                    gpu.size.height,
                );

                let surface_texture = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(t) => t,
                    wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                        gpu.surface.configure(&gpu.device, &gpu.config);
                        t
                    }
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.config);
                        return;
                    }
                    // A future wgpu release may introduce new variants. Skip
                    // the frame but make the skip visible — silently dropping
                    // frames here masked a real OS-level surface failure once.
                    other => {
                        log::warn!("unexpected surface state: {other:?}; skipping frame");
                        return;
                    }
                };

                let view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    gpu.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Render Encoder"),
                        });

                gpu.renderer.render(&mut encoder, &view);
                gpu.queue.submit(std::iter::once(encoder.finish()));
                surface_texture.present();

                if let Some(window) = &self.window {
                    window.request_redraw();
                } else {
                    log::warn!("redraw completed but viewer window is missing");
                }
            }

            _ => {}
        }
    }
}
