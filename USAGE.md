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

`crates/common` is deliberately outside the engine tier despite living under
`crates/`: native hosts use it for `clap`/`chrono` parsing and shared
catalog→renderer adapter helpers, while `apps/web` talks to the engine crates
directly to keep the WASM path free of native-only dependencies.

Coordinate conventions (all crates agree):

- Time is represented by `astronomy::TimeScales`: UTC for civil input, UT1
  for Earth rotation / sidereal time, TAI and TT from the built-in leap-second
  table, and an approximate TDB for ephemerides. DUT1 defaults to zero unless a
  caller supplies it, so unknown UT1−UTC can still move sidereal quantities by
  up to about 13.5 arcsec.
- Catalog stars are **J2000/ICRS-like unit-sphere Cartesian**:
  `x = cos δ cos α, y = cos δ sin α, z = sin δ`. Precession, nutation,
  aberration, and proper motion are not applied yet.
- The current Sun/Moon helpers are low-precision apparent/topocentric visual
  inputs for rendering daylight, moonlight, and disks. They are not the final
  VSOP87/ELP2000, IAU-2006/2000A precision stack tracked in README Phase 2.
- Local frame is **ENU** (East, North, Up). Observer latitude is treated as
  geodetic for topocentric solar-system parallax and as astronomical/geographic
  for stellar ENU projection; the distinction is below Phase 1 star precision.
- Azimuth is from **North toward East**. Catalog and overlay altitudes are
  geometric by definition; when the renderer's atmosphere is enabled, the star
  shader applies a standard-pressure/temperature Saemundsson-style stellar
  refraction correction before projection. Refraction for the Sun/Moon disks
  and configurable weather inputs remain Phase 2 follow-up work.

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
use renderer::{build_star_instance, StarInstance, NAKED_EYE_LIMITING_MAGNITUDE};

let stars = load_from_file("crates/catalog/data/hyg_v42.csv")?;
//   or:  let stars = catalog::load_embedded();           // embedded feature
//   or:  let stars = catalog::load_from_csv(csv_string); // any source

// Faintest magnitude the simulated observer can still see. 6.0 is the
// strict dark-adapted naked-eye limit; bump it (e.g. 7.5) on indoor screens
// whose dynamic range can't reproduce a dark sky faithfully.
let limiting_magnitude = NAKED_EYE_LIMITING_MAGNITUDE + 1.5;

let instances: Vec<StarInstance> = stars
    .iter()
    .map(|s| build_star_instance(s.position.into(), s.color, s.magnitude, limiting_magnitude))
    .collect();
```

Native reference hosts use `stars_host_common::load_star_instances_from_file`
for this catalog→renderer bridge so their filesystem catalog loading and
perceptual star-instance conversion cannot drift. New non-native hosts can do
the explicit conversion above with whichever catalog backend they own.

Catalog filtering happens inside `catalog`: rows fainter than magnitude 8 and
rows whose `dist` (parsecs) is HYG's `100000` sentinel-for-unknown-parallax
are dropped automatically. Get the CSV with `./scripts/download-catalog.sh`
(writes to `crates/catalog/data/hyg_v42.csv`).

### 4. Stand up wgpu, then build a `Renderer` + `Camera`

The renderer is platform-agnostic — give it any `Device`, surface
`TextureFormat`, and the instance buffer. Surface acquisition is the host's
job (canvas, window, headless texture).

```rust
use renderer::{Camera, LocalView, Renderer};

let renderer = Renderer::new(&device, surface_format, width, height, &instances);

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

On `Resized` events: reconfigure the surface, set
`camera.aspect = w as f32 / h as f32`, and call
`renderer.resize(&device, w, h)` so the internal HDR target tracks the
swapchain. The renderer reads the new viewport through `update_camera`.

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
radec_hours_deg_to_cartesian(ra_hours, dec_degrees) -> Vec3
struct Star { position: Vec3, magnitude: f32, color: [f32; 3] }

// renderer
Renderer::new(&device, format, width, height, &[StarInstance]) -> Renderer
Renderer::set_overlays(&device, &OverlayConfig)
Renderer::update_camera(&queue, &Camera, w, h)
Renderer::render(&mut encoder, &TextureView)

Camera::new(observer, LocalView, aspect) -> Camera
Camera::rotate_view(daz_rad, dalt_rad)
Camera::zoom_fov(factor)

LocalView { azimuth_rad, altitude_rad, fov_y_rad }
StarInstance { position: [f32;3], size: f32, color: [f32;3], brightness: f32 }
magnitude_to_render_params(mag, limiting_magnitude) -> RenderParams { radius_px, brightness }
NAKED_EYE_LIMITING_MAGNITUDE: f32      // 6.0; literature default
DEFAULT_SCREEN_LIMITING_MAGNITUDE: f32 // 7.5; screen-host default

// Overlays (all routed through OverlayKind::as_kebab_str / from_kebab_str)
OverlayConfig { layers: Vec<OverlayKind>, grid_step_deg: f64, opacity: f32 }
enum OverlayKind { Horizon, Cardinals, AltAzGrid, EquatorialGrid,
                   Ecliptic, CelestialEquator, Meridian, GalacticEquator,
                   ConstellationLines, ConstellationBoundaries }
```
