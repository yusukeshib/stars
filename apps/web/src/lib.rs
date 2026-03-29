use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use stars_catalog::catalog;
use stars_core::camera::Camera;
use stars_core::renderer::Renderer;
use stars_core::vertex::{magnitude_to_size, StarInstance};

use std::cell::RefCell;
use std::rc::Rc;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();
}

#[wasm_bindgen]
pub async fn start_renderer(canvas_id: String) -> Result<(), JsValue> {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let canvas = document
        .get_element_by_id(&canvas_id)
        .ok_or("Canvas not found")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let dpr = window.device_pixel_ratio();
    let width = (canvas.client_width() as f64 * dpr) as u32;
    let height = (canvas.client_height() as f64 * dpr) as u32;
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
    let stars = catalog::load_embedded();
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
    let camera = Camera::new(width as f32 / height as f32);

    let state = Rc::new(RefCell::new(RenderState {
        surface,
        device,
        queue,
        config,
        renderer,
        camera,
        width,
        height,
        mouse_pressed: false,
        last_mouse: None,
    }));

    setup_mouse_events(&canvas, state.clone())?;
    start_render_loop(state);

    Ok(())
}

struct RenderState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    camera: Camera,
    width: u32,
    height: u32,
    mouse_pressed: bool,
    last_mouse: Option<(f64, f64)>,
}

fn setup_mouse_events(
    canvas: &web_sys::HtmlCanvasElement,
    state: Rc<RefCell<RenderState>>,
) -> Result<(), JsValue> {
    {
        let state = state.clone();
        let cb = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
            state.borrow_mut().mouse_pressed = true;
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    {
        let state = state.clone();
        let cb = Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
            let mut s = state.borrow_mut();
            s.mouse_pressed = false;
            s.last_mouse = None;
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mouseup", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    {
        let state = state.clone();
        let cb = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
            let mut s = state.borrow_mut();
            if s.mouse_pressed {
                let x = e.client_x() as f64;
                let y = e.client_y() as f64;
                if let Some((lx, ly)) = s.last_mouse {
                    let dx = (x - lx) as f32 * 0.005;
                    let dy = (y - ly) as f32 * 0.005;
                    s.camera.rotate(dx, -dy);
                }
                s.last_mouse = Some((x, y));
            }
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("mousemove", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    {
        let state = state.clone();
        let cb = Closure::wrap(Box::new(move |e: web_sys::WheelEvent| {
            e.prevent_default();
            let delta = e.delta_y() as f32 * -0.01;
            state.borrow_mut().camera.zoom(delta);
        }) as Box<dyn FnMut(_)>);
        canvas.add_event_listener_with_callback("wheel", cb.as_ref().unchecked_ref())?;
        cb.forget();
    }
    Ok(())
}

fn start_render_loop(state: Rc<RefCell<RenderState>>) {
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        {
            let s = state.borrow();
            s.renderer
                .update_camera(&s.queue, &s.camera, s.width, s.height);

            let surface_texture = match s.surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(t)
                | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                _ => return,
            };
            let view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder =
                s.device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Render Encoder"),
                    });
            s.renderer.render(&mut encoder, &view);
            s.queue.submit(std::iter::once(encoder.finish()));
            surface_texture.present();
        }

        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .unwrap();
}
