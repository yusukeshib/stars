use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use astronomy::{julian_date_from_unix_seconds, Observer};
use catalog::load_from_file;
use clap::Parser;
use renderer::{magnitude_to_render_params, Camera, LocalView, Renderer, StarInstance};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

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

    /// Initial vertical field of view, degrees.
    #[arg(long, default_value_t = 70.0)]
    fov: f64,

    /// Path to the HYG-format star catalog CSV.
    #[arg(long, default_value = "crates/catalog/data/hyg_v42.csv")]
    catalog: PathBuf,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    let start_jd = parse_time_to_jd(args.time.as_deref())?;
    let stars = load_from_file(&args.catalog)
        .with_context(|| format!("Reading catalog at {}", args.catalog.display()))?;
    log::info!("Loaded {} stars", stars.len());
    let instances: Vec<StarInstance> = stars
        .iter()
        .map(|s| {
            let p = magnitude_to_render_params(s.magnitude);
            StarInstance {
                position: s.position.into(),
                size: p.radius_px,
                color: s.color,
                brightness: p.brightness,
            }
        })
        .collect();

    let initial_view = LocalView {
        azimuth_rad: (args.azimuth as f32).to_radians(),
        altitude_rad: (args.altitude as f32).to_radians(),
        fov_y_rad: (args.fov as f32).to_radians(),
    };

    let event_loop = EventLoop::new()?;
    let mut app = App::new(instances, args.lat, args.lng, start_jd, initial_view);
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn parse_time_to_jd(time: Option<&str>) -> Result<f64> {
    let unix_seconds = match time {
        Some(s) => {
            let dt = chrono::DateTime::parse_from_rfc3339(s)
                .with_context(|| format!("Invalid RFC3339 time: {s}"))?;
            dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 * 1e-9
        }
        None => {
            let now = chrono::Utc::now();
            now.timestamp() as f64 + now.timestamp_subsec_nanos() as f64 * 1e-9
        }
    };
    Ok(julian_date_from_unix_seconds(unix_seconds))
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
            speed: 1.0,
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
    fn new(
        stars: Vec<StarInstance>,
        lat: f64,
        lng: f64,
        start_jd: f64,
        initial_view: LocalView,
    ) -> Self {
        Self {
            gpu: None,
            window: None,
            stars,
            lat,
            lng,
            initial_view,
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
            .with_inner_size(PhysicalSize::new(1280u32, 720u32));
        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        self.window = Some(window.clone());

        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("No suitable GPU adapter found");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Stars Viewer Device"),
            ..Default::default()
        }))
        .expect("Failed to create device");

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

        let renderer = Renderer::new(&device, format, &self.stars);
        let observer = Observer::from_degrees(self.lat, self.lng, self.sky_clock.current_jd());
        let camera = Camera::new(
            observer,
            self.initial_view,
            size.width as f32 / size.height as f32,
        );

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
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.01,
                };
                gpu.camera.zoom_fov((-scroll * 0.05).exp());
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
                KeyCode::Digit1 => self.sky_clock.set_speed(1.0),
                KeyCode::Digit2 => self.sky_clock.set_speed(60.0),
                KeyCode::Digit3 => self.sky_clock.set_speed(3600.0),
                KeyCode::Digit4 => self.sky_clock.set_speed(86_400.0),
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
                    _ => return,
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

                self.window.as_ref().unwrap().request_redraw();
            }

            _ => {}
        }
    }
}
