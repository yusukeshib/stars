use std::cell::RefCell;
use std::rc::Rc;

use astronomy::{
    apparent_sun_topocentric, equatorial_to_horizontal, lmst_radians, Observer, TimeScales,
};
use catalog::load_embedded;
use renderer::{
    build_star_instance, Atmosphere, AtmospherePreset, Camera, LocalView, OverlayConfig,
    OverlayKind, Renderer, StarInstance, DEFAULT_SCREEN_LIMITING_MAGNITUDE,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Slightly past strict naked-eye to compensate for typical monitor viewing
/// conditions (the on-screen dynamic range is much smaller than a dark-adapted
/// observer's). See `renderer::magnitude_to_render_params` for the model.
const LIMITING_MAGNITUDE: f32 = DEFAULT_SCREEN_LIMITING_MAGNITUDE;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();
}

struct RenderState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    camera: Camera,
}

#[wasm_bindgen]
pub struct StarView {
    state: Rc<RefCell<RenderState>>,
}

#[wasm_bindgen]
impl StarView {
    /// Create a renderer attached to the canvas with the given DOM id.
    pub async fn create(canvas_id: String) -> Result<StarView, JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let canvas = document
            .get_element_by_id(&canvas_id)
            .ok_or("Canvas not found")?
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        let dpr = window.device_pixel_ratio();
        let width = ((canvas.client_width() as f64) * dpr).max(1.0) as u32;
        let height = ((canvas.client_height() as f64) * dpr).max(1.0) as u32;
        canvas.set_width(width);
        canvas.set_height(height);

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsValue::from_str(&format!("Surface error: {e}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Adapter error: {e}")))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Stars Device"),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Device error: {e}")))?;

        let surface_caps = surface.get_capabilities(&adapter);
        // Prefer an sRGB surface so the hardware applies the linear→sRGB EOTF
        // on present. The star shader emits linear radiance (Pogson's law, see
        // `vertex::magnitude_to_render_params`); writing that straight into a
        // non-sRGB framebuffer crushes mid/faint magnitudes to near-black on
        // the display. Selecting an sRGB format is the correct, lossless way
        // to get perceptually right brightness without altering the physics.
        let format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        log::info!("Surface format: {format:?} (sRGB: {})", format.is_srgb());
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        log::info!("Loading star catalog...");
        let stars = load_embedded();
        log::info!("Loaded {} stars", stars.len());
        let instances: Vec<StarInstance> = stars
            .iter()
            .map(|s| build_star_instance(s.position.into(), s.color, s.magnitude, LIMITING_MAGNITUDE))
            .collect();

        let renderer = Renderer::new(&device, format, width, height, &instances);
        let mut camera = Camera::new(
            // Defaults; JS will overwrite immediately.
            Observer::from_degrees(0.0, 0.0, 2_451_545.0),
            LocalView::default(),
            width as f32 / height as f32,
        );
        // Same brightness scale as the star pipeline so the skyglow pass
        // composites correctly on top.
        camera.limiting_magnitude = LIMITING_MAGNITUDE;

        Ok(StarView {
            state: Rc::new(RefCell::new(RenderState {
                surface,
                device,
                queue,
                config,
                renderer,
                camera,
            })),
        })
    }

    /// Update the observer. `time_unix_ms` is a JS `Date.now()`-style millisecond
    /// epoch; conversion to Julian Date happens here so the JS side doesn't need
    /// to know the constant.
    pub fn set_observer(&self, lat_deg: f64, lng_deg: f64, time_unix_ms: f64) {
        let time = TimeScales::from_unix_seconds(time_unix_ms / 1000.0);
        self.state.borrow_mut().camera.observer =
            Observer::from_degrees_with_time(lat_deg, lng_deg, time);
    }

    /// Current apparent topocentric Sun altitude, in degrees.
    ///
    /// The HUD uses this for daylight/twilight labels so the user-visible sky
    /// state is derived from the same Rust ephemeris and `TimeScales` convention as
    /// the renderer's daylight, twilight, and disk inputs. Keeping the formula
    /// here avoids a second, drifting JavaScript solar-position implementation.
    pub fn sun_altitude_deg(&self) -> f64 {
        let observer = self.state.borrow().camera.observer;
        let sun = apparent_sun_topocentric(observer);
        let lst = lmst_radians(observer.time.jd_ut1, observer.longitude_rad);
        equatorial_to_horizontal(
            sun.right_ascension_rad,
            sun.declination_rad,
            lst,
            observer.latitude_rad,
        )
        .altitude
        .to_degrees()
    }

    /// Update the active overlay layers. `layers` is a list of kebab-case names
    /// that match the CLI's `--overlays` flag: "horizon", "cardinals",
    /// "alt-az-grid", "equatorial-grid", "ecliptic", "celestial-equator",
    /// "meridian", "galactic-equator". Unknown names are ignored with a
    /// warning so the JS layer can evolve without breaking older builds.
    ///
    /// `grid_step_deg` and `opacity` are passed through to the renderer, which
    /// applies its own clamps; finite values outside the renderer's accepted
    /// range are silently coerced. Non-finite values would propagate into the
    /// geometry generators and produce NaN vertices, so we replace them with
    /// the renderer's defaults here.
    pub fn set_overlays(&self, layers: Vec<String>, grid_step_deg: f64, opacity: f32) {
        let kinds: Vec<OverlayKind> = layers
            .iter()
            .filter_map(|name| {
                let parsed = OverlayKind::from_kebab_str(name);
                if parsed.is_none() {
                    log::warn!("unknown overlay name from JS: {name:?}");
                }
                parsed
            })
            .collect();
        let grid_step_deg = if grid_step_deg.is_finite() {
            grid_step_deg
        } else {
            15.0
        };
        let opacity = if opacity.is_finite() { opacity } else { 0.6 };
        let s = &mut *self.state.borrow_mut();
        s.renderer.set_overlays(
            &s.device,
            &OverlayConfig {
                layers: kinds,
                grid_step_deg,
                opacity,
            },
        );
    }

    pub fn set_view(&self, azimuth_rad: f32, altitude_rad: f32, fov_y_rad: f32) {
        self.state.borrow_mut().camera.view = LocalView {
            azimuth_rad,
            altitude_rad,
            fov_y_rad,
        }
        .clamped();
    }

    /// Update atmosphere controls from the web UI. `enabled=false` matches the
    /// native `--no-extinction` flag and disables both extinction and sunlit
    /// scattering.
    pub fn set_atmosphere(&self, enabled: bool, turbidity: f32, observer_altitude_m: f32) {
        self.state.borrow_mut().camera.atmosphere = if enabled {
            Atmosphere {
                turbidity,
                observer_altitude_m,
                ..Atmosphere::default()
            }
        } else {
            Atmosphere::OFF
        };
    }

    /// Select one of the renderer's serializable atmosphere presets by kebab
    /// name: `clear-rural`, `hazy-urban`, or `high-altitude`.
    pub fn set_atmosphere_preset(&self, enabled: bool, preset: String) {
        self.state.borrow_mut().camera.atmosphere = if enabled {
            AtmospherePreset::from_kebab_str(&preset)
                .map(Atmosphere::from_preset)
                .unwrap_or_default()
        } else {
            Atmosphere::OFF
        };
    }

    /// Update the complete atmosphere state from the web UI: preset controls
    /// extinction coefficients, while turbidity / altitude remain user-tunable.
    pub fn set_atmosphere_config(
        &self,
        enabled: bool,
        preset: String,
        turbidity: f32,
        observer_altitude_m: f32,
        ozone_du: f32,
        visibility_km: f32,
    ) {
        self.state.borrow_mut().camera.atmosphere = if enabled {
            let mut atmosphere = AtmospherePreset::from_kebab_str(&preset)
                .map(Atmosphere::from_preset)
                .unwrap_or_default();
            atmosphere.turbidity = turbidity;
            atmosphere.observer_altitude_m = observer_altitude_m;
            atmosphere.ozone_du = ozone_du;
            atmosphere.visibility_km = visibility_km;
            atmosphere
        } else {
            Atmosphere::OFF
        };
    }

    pub fn resize(&self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        let mut s = self.state.borrow_mut();
        s.config.width = width;
        s.config.height = height;
        s.surface.configure(&s.device, &s.config);
        s.camera.aspect = width as f32 / height as f32;
        // Keep the renderer's HDR target matched to the swapchain.
        // Split-borrow: the borrow checker can't see that `device` and
        // `renderer` don't alias when accessed through one `&mut s`.
        let RenderState {
            device, renderer, ..
        } = &mut *s;
        renderer.resize(device, width, height);
    }

    /// Render a single frame using the current observer/view.
    pub fn render_frame(&self) {
        let s = self.state.borrow();
        s.renderer
            .update_camera(&s.queue, &s.camera, s.config.width, s.config.height);

        let surface_texture = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                log::warn!("surface texture was outdated/lost; reconfiguring surface");
                s.surface.configure(&s.device, &s.config);
                return;
            }
            unexpected => {
                log::warn!("skipping frame after unexpected surface texture state: {unexpected:?}");
                return;
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = s
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        s.renderer.render(&mut encoder, &view);
        s.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
    }
}
