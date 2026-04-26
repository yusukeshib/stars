use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use stars_astronomy::julian_date_from_unix_seconds;
use stars_catalog::catalog;
use stars_core::camera::{Camera, LocalView, Observer};
use stars_core::renderer::Renderer;
use stars_core::vertex::{magnitude_to_size, StarInstance};
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

    /// Path to the HYG-format star catalog CSV.
    #[arg(long, default_value = "crates/stars-catalog/data/hyg_v42.csv")]
    catalog: PathBuf,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    let start_jd = parse_time_to_jd(args.time.as_deref())?;
    let stars = catalog::load_from_file(args.catalog.to_str().unwrap());
    log::info!("Loaded {} stars", stars.len());
    let instances: Vec<StarInstance> = stars
        .iter()
        .map(|s| StarInstance {
            position: s.position.into(),
            size: magnitude_to_size(s.magnitude),
            color: s.color,
            _pad: 0.0,
        })
        .collect();

    let event_loop = EventLoop::new()?;
    let mut app = App::new(instances, args.lat, args.lng, start_jd);
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
    start_jd: f64,
    /// Wall-clock instant corresponding to `start_jd`. Sky time = start_jd + (now - epoch) * speed.
    epoch: Instant,
    /// Multiplier on real-time elapsed seconds. 1.0 = real time, 60.0 = a minute per second.
    time_speed: f64,
    paused: bool,
    paused_jd: f64,
    mouse_pressed: bool,
    last_mouse: Option<(f64, f64)>,
}

impl App {
    fn new(stars: Vec<StarInstance>, lat: f64, lng: f64, start_jd: f64) -> Self {
        Self {
            gpu: None,
            window: None,
            stars,
            lat,
            lng,
            start_jd,
            epoch: Instant::now(),
            time_speed: 1.0,
            paused: false,
            paused_jd: start_jd,
            mouse_pressed: false,
            last_mouse: None,
        }
    }

    fn current_jd(&self) -> f64 {
        if self.paused {
            self.paused_jd
        } else {
            let elapsed_seconds = self.epoch.elapsed().as_secs_f64();
            self.start_jd + (elapsed_seconds * self.time_speed) / 86_400.0
        }
    }

    fn set_speed(&mut self, speed: f64) {
        // Re-anchor so the visible time doesn't jump.
        let now_jd = self.current_jd();
        self.start_jd = now_jd;
        self.epoch = Instant::now();
        self.time_speed = speed;
    }

    fn toggle_pause(&mut self) {
        if self.paused {
            self.start_jd = self.paused_jd;
            self.epoch = Instant::now();
            self.paused = false;
        } else {
            self.paused_jd = self.current_jd();
            self.paused = true;
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

        let renderer = Renderer::new(&device, &queue, format, &self.stars);
        let observer = Observer::from_degrees(self.lat, self.lng, self.current_jd());
        let view = LocalView {
            azimuth_rad: std::f32::consts::PI, // facing south
            altitude_rad: 30.0_f32.to_radians(),
            fov_y_rad: 70.0_f32.to_radians(),
        };
        let camera = Camera::new(observer, view, size.width as f32 / size.height as f32);

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
        let jd = self.current_jd();
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
                KeyCode::Space => self.toggle_pause(),
                KeyCode::Digit1 => self.set_speed(1.0),
                KeyCode::Digit2 => self.set_speed(60.0),
                KeyCode::Digit3 => self.set_speed(3600.0),
                KeyCode::Digit4 => self.set_speed(86_400.0),
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
