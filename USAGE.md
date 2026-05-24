# Using the `stars` engine in a new implementation

This repo is a Rust workspace that splits a star-rendering engine into three
small library crates, plus three reference host apps (`cli`, `viewer`, `web`)
that consume them. Any new host — a mobile app, a headless server, a test
harness, an alternate UI — follows the **same 5-step recipe** below.

If you just want to read the existing hosts as templates:

- **Headless / one-shot render** → [`apps/cli/src/main.rs`](apps/cli/src/main.rs)
- **Interactive desktop window** → [`apps/viewer/src/main.rs`](apps/viewer/src/main.rs)
- **Browser / WASM**           → [`apps/web/src/lib.rs`](apps/web/src/lib.rs)

---

## Architecture at a glance

```
┌────────────────────────┐    ┌───────────────────────────────┐
│ astronomy              │    │ catalog                       │
│  Observer              │    │  Star { position, mag, color} │
│  julian_date_…         │    │  load_from_file (fs feature)  │
│  lmst_radians          │    │  load_embedded  (embedded ft.)│
│  equatorial_to_horiz…  │    │  load_from_csv  (always)      │
└──────────┬─────────────┘    └──────────────┬────────────────┘
           │                                 │
           └───────────────┬─────────────────┘
                           ▼
              ┌──────────────────────────────┐
              │ renderer (wgpu)              │
              │  Camera { observer, view }   │
              │  StarInstance                │
              │  magnitude_to_render_params  │
              │  Renderer::new / render      │
              └──────────────┬───────────────┘
                             │
       ┌─────────────────────┼─────────────────────┐
       ▼                     ▼                     ▼
   apps/cli              apps/viewer            apps/web
   (PNG output)          (winit window)         (WASM + canvas)
```

Coordinate conventions (all crates agree):

- Time is **Julian Date in UT** (`UT1 ≈ UTC`).
- Equatorial positions are **J2000 unit-sphere Cartesian**:
  `x = cos δ cos α, y = cos δ sin α, z = sin δ`.
- Local frame is **ENU** (East, North, Up).
- Azimuth is from **North toward East**; altitude is **above horizon** (radians).

---

## The 5-step recipe

### 1. Add the crates to your `Cargo.toml`

Pick the catalog backend that matches your platform:

```toml
[dependencies]
astronomy = { path = "../../crates/astronomy" }
renderer  = { path = "../../crates/renderer"  }

# Native host (CLI, desktop, tests): reads the CSV from disk.
catalog   = { path = "../../crates/catalog", features = ["filesystem"] }

# WASM / single-binary deployments: bakes the CSV into the .wasm/.exe.
# catalog = { path = "../../crates/catalog", default-features = false, features = ["embedded"] }

wgpu = "29"
```

`filesystem` and `embedded` are **mutually exclusive in spirit** — pick one
based on whether you can read files at runtime.

### 2. Build the `Observer`

```rust
use astronomy::{julian_date_from_unix_seconds, Observer};

let unix_seconds = /* e.g. chrono::Utc::now().timestamp() as f64 */;
let jd = julian_date_from_unix_seconds(unix_seconds);

let observer = Observer::from_degrees(
    35.68,    // latitude  (north positive)
    139.69,   // longitude (east  positive)
    jd,
);
```

For interactive hosts, refresh `observer.julian_date` (or rebuild the
`Observer`) on every frame so the sky tracks real time. See `SkyClock` in
[`apps/viewer/src/main.rs`](apps/viewer/src/main.rs) for a pause/speed-up
clock you can reuse.

### 3. Load the catalog and convert to `StarInstance`s

```rust
use catalog::{load_from_file /* or load_embedded */};
use renderer::{magnitude_to_render_params, StarInstance};

let stars = load_from_file("crates/catalog/data/hyg_v42.csv")?;
//   or:  let stars = catalog::load_embedded();           // embedded feature
//   or:  let stars = catalog::load_from_csv(csv_string); // any source

let instances: Vec<StarInstance> = stars
    .iter()
    .map(|s| {
        let p = magnitude_to_render_params(s.magnitude);
        StarInstance {
            position:   s.position.into(),
            size:       p.radius_px,
            color:      s.color,
            brightness: p.brightness,
        }
    })
    .collect();
```

Catalog filtering happens inside `catalog`: rows fainter than magnitude 8 and
rows further than 100 kpc are dropped automatically. Get the CSV with
`./scripts/download-catalog.sh` (writes to `crates/catalog/data/hyg_v42.csv`).

### 4. Stand up wgpu, then build a `Renderer` + `Camera`

The renderer is platform-agnostic — give it any `Device`, surface
`TextureFormat`, and the instance buffer. Surface acquisition is the host's
job (canvas, window, headless texture).

```rust
use renderer::{Camera, LocalView, Renderer};

let renderer = Renderer::new(&device, surface_format, &instances);

let view = LocalView {
    azimuth_rad:  180_f32.to_radians(),
    altitude_rad:  30_f32.to_radians(),
    fov_y_rad:     70_f32.to_radians(),
};
let mut camera = Camera::new(observer, view, width as f32 / height as f32);
```

`LocalView` is your camera's orientation in the observer's local horizontal
frame. `Camera` knows how to combine it with `Observer` into a view-projection
matrix (`Camera::view_proj`), and exposes two helpers for interactive hosts:

- `camera.rotate_view(daz, dalt)` — drag-rotate, altitude clamped near ±π/2.
- `camera.zoom_fov(factor)`       — multiplicative FOV zoom, clamped 5°–120°.

### 5. Render loop

Per frame:

```rust
// 1. Push latest observer/view to the GPU.
camera.observer.julian_date = current_jd;           // if time advances
renderer.update_camera(&queue, &camera, width, height);

// 2. Acquire a target texture view from your surface / headless texture.
let view = /* surface_texture.create_view(...) */;

// 3. Encode and submit.
let mut encoder = device.create_command_encoder(&Default::default());
renderer.render(&mut encoder, &view);
queue.submit([encoder.finish()]);

// 4. Present (windowed hosts only).
surface_texture.present();
```

On `Resized` events: reconfigure the surface and set
`camera.aspect = w as f32 / h as f32`. The renderer reads the new viewport
through `update_camera`, so nothing else needs touching.

---

## Host-specific notes

### Headless / image output (`apps/cli`)

- Create the target as a regular `wgpu::Texture` with
  `RENDER_ATTACHMENT | COPY_SRC` usage.
- After `renderer.render(...)`, `copy_texture_to_buffer` into a mappable
  buffer using **`COPY_BYTES_PER_ROW_ALIGNMENT`-padded rows**, then strip
  the padding back out before handing to `image::RgbaImage::from_raw`.
- Use `pollster::block_on` to drive the async wgpu calls from `main`.

### Windowed (`apps/viewer`)

- `winit 0.30` uses `ApplicationHandler`; defer all wgpu init to `resumed`.
- Wrap `Window` in `Arc` so the wgpu surface can outlive the event handler.
- Prefer an `is_srgb()` surface format; the renderer's clear color and
  blending assume sRGB output.
- Pixel-drag → angle conversion that feels right at any zoom:
  `daz = -dx * fov_y / viewport_height` (see `WindowEvent::CursorMoved`).

### WASM / browser (`apps/web`)

- The crate must be `crate-type = ["cdylib", "rlib"]`.
- Use the `embedded` feature on `catalog` so no fetch is needed.
- The exposed `StarView` keeps a `Rc<RefCell<RenderState>>` so JS can call
  `set_observer` / `set_view` / `render_frame` independently.
- Multiply by `window.devicePixelRatio` when sizing the canvas backing store,
  and call `resize` from a `window.resize` listener.

---

## Build, test, lint

The `Makefile` is the single source of truth:

```bash
make setup     # download catalog + install web deps
make cli       # run CLI (override ARGS="--lat … --lng … -o sky.png")
make viewer    # run interactive desktop viewer
make web       # build WASM and start the Vite dev server
make ci        # fmt --check, clippy -D warnings, tests, wasm check
```

When adding a new host:

1. Add it to `members = [...]` in the workspace `Cargo.toml`
   (unless it only targets `wasm32-unknown-unknown` — then `exclude` it like
   `apps/web` and add a separate `cargo check --target …` line to CI).
2. Add a `make <host>` target.
3. Add a job (or step) to `.github/workflows/ci.yml`.

---

## Quick API reference

```rust
// astronomy
Observer::from_degrees(lat_deg, lng_deg, jd) -> Observer
julian_date_from_unix_seconds(secs) -> f64
gmst_radians(jd) -> f64
lmst_radians(jd, longitude_east_rad) -> f64
equatorial_to_horizontal(ra, dec, lst, lat) -> AltAz
equatorial_to_horizontal_matrix(lat, lst) -> Mat4

// catalog
load_from_csv(&str) -> Vec<Star>
load_from_file(path) -> io::Result<Vec<Star>>   // feature = "filesystem"
load_embedded() -> Vec<Star>                    // feature = "embedded"
bv_to_rgb(bv) -> [f32; 3]
radec_to_cartesian(ra_hours, dec_degrees) -> Vec3
struct Star { position: Vec3, magnitude: f32, color: [f32; 3] }

// renderer
Renderer::new(&device, format, &[StarInstance]) -> Renderer
Renderer::update_camera(&queue, &Camera, w, h)
Renderer::render(&mut encoder, &TextureView)

Camera::new(observer, LocalView, aspect) -> Camera
Camera::rotate_view(daz_rad, dalt_rad)
Camera::zoom_fov(factor)

LocalView { azimuth_rad, altitude_rad, fov_y_rad }
StarInstance { position: [f32;3], size: f32, color: [f32;3], brightness: f32 }
magnitude_to_render_params(mag) -> RenderParams { radius_px, brightness }
```
