use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use catalog::load_embedded;
use renderer::{magnitude_to_size, Camera, LocalView, Observer, Renderer, StarInstance};

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
        let format = surface_caps.formats[0];
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
            .map(|s| StarInstance {
                position: s.position.into(),
                size: magnitude_to_size(s.magnitude),
                color: s.color,
                _pad: 0.0,
            })
            .collect();

        let renderer = Renderer::new(&device, &queue, format, &instances);
        let camera = Camera::new(
            // Defaults; JS will overwrite immediately.
            Observer::from_degrees(0.0, 0.0, 2_451_545.0),
            LocalView::default(),
            width as f32 / height as f32,
        );

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

    pub fn set_observer(&self, lat_deg: f64, lng_deg: f64, julian_date: f64) {
        let mut s = self.state.borrow_mut();
        s.camera.observer = Observer::from_degrees(lat_deg, lng_deg, julian_date);
    }

    pub fn set_view(&self, azimuth_rad: f32, altitude_rad: f32, fov_y_rad: f32) {
        let mut s = self.state.borrow_mut();
        s.camera.view = LocalView {
            azimuth_rad,
            altitude_rad,
            fov_y_rad,
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
    }

    /// Render a single frame using the current observer/view.
    pub fn render_frame(&self) -> Result<(), JsValue> {
        let s = self.state.borrow();
        s.renderer
            .update_camera(&s.queue, &s.camera, s.config.width, s.config.height);

        let surface_texture = match s.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                s.surface.configure(&s.device, &s.config);
                return Ok(());
            }
            _ => return Ok(()),
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
        Ok(())
    }
}
