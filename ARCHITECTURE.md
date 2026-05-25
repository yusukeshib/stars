# Architecture

This document explains how the `stars` engine is split, how data flows through
it, and how a new host should integrate the renderer. It replaces the old
`USAGE.md`: this is not just user-facing usage, but the architectural contract
between engine crates and host applications.

## High-level shape

```txt
┌─────────────────────────────┐      ┌─────────────────────────────┐
│ crates/astronomy            │      │ crates/catalog              │
│  time scales                │      │  HYG CSV / embedded catalog │
│  observer / local frame     │      │  Star records               │
│  corrections                │      │  B-V colour conversion      │
│  Sun/Moon/planet apparent   │      │  proper-motion vectors      │
│  atmosphere / skyglow       │      └──────────────┬──────────────┘
│  planning helpers           │                     │
└──────────────┬──────────────┘                     │
               │                                    │
               └────────────────┬───────────────────┘
                                ▼
                    ┌─────────────────────────┐
                    │ crates/renderer         │
                    │  Camera / LocalView     │
                    │  StarInstance           │
                    │  overlays + text labels │
                    │  skyglow + atmosphere   │
                    │  HDR + tonemap          │
                    │  Renderer::render       │
                    └────────────┬────────────┘
                                 │
          ┌──────────────────────┼──────────────────────┐
          ▼                      ▼                      ▼
      apps/cli               apps/viewer             apps/web
      PNG output             native window           WASM + canvas
```

`crates/common` is intentionally outside the engine tier even though it lives
under `crates/`. It contains native-host glue: `clap` mirrors of renderer enums,
`chrono` time parsing, atmosphere / overlay argument mapping, and shared
catalog-to-renderer conversion for CLI and desktop. The web host bypasses it so
WASM stays free of native-only dependencies.

## Crate boundaries

### `crates/astronomy`

Owns scientific and geometric models:

- civil and dynamical time scales: UTC, UT1, TAI, TT, approximate TDB;
- `Observer` and local sidereal-time helpers;
- equatorial-to-horizontal transforms;
- proper-motion epoch math, annual aberration, IAU 2006 precession, compact
  IAU-2000-style nutation, and atmospheric refraction;
- Sun, Moon, and planet apparent/topocentric helpers;
- photometry, illuminants, atmosphere, twilight, and skyglow reference models;
- planning helpers such as rise / transit / set and twilight bands.

This crate should not depend on a renderer, a UI framework, or host-specific
argument parsing.

### `crates/catalog`

Owns star catalog ingestion and catalog-space conversions:

- `load_from_csv(&str)` for generic CSV content;
- `load_from_file(path)` behind the `filesystem` feature;
- `load_embedded()` behind the `embedded` feature;
- HYG row filtering and conversion into `Star` records;
- B−V colour conversion and RA/Dec-to-Cartesian helpers.

Catalog stars are renderer-independent. They should not know about `wgpu`,
window size, camera state, or UI labels.

### `crates/renderer`

Owns GPU-facing rendering state:

- `Renderer` lifecycle and render passes;
- `Camera`, `LocalView`, `SkyProjection`, `SkyViewpoint`, and GPU camera uniforms;
- `StarInstance` and `build_star_instance`;
- overlay geometry, text labels, and `OverlayKind` / `OverlayConfig`;
- HDR target, skyglow pass, tonemap pass, star shader, text shader, and atmosphere uniforms.

The renderer is platform-agnostic. It expects the host to provide a `wgpu::Device`,
queue, target format, target `TextureView`, and resize / surface management.

### `crates/common`

Owns native-host convenience only:

- CLI-facing mirrors of renderer enums;
- native atmosphere / overlay argument conversion;
- RFC3339 / now time parsing;
- filesystem catalog loading plus conversion to renderer `StarInstance`s.

Do not put core astronomical or rendering logic here.

### `apps/*`

Hosts own platform lifecycle:

- `apps/cli`: create a headless texture, render once, copy padded rows back to
  CPU memory, and write PNG;
- `apps/viewer`: manage a `winit` event loop, surface resize, input, and frame
  pacing;
- `apps/web`: expose a WASM `StarView`, keep JS/UI state, resize the canvas,
  and call into the shared renderer.

## Coordinate and time conventions

All crates should preserve these conventions:

- Angles are radians internally unless a helper explicitly says degrees or
  hours.
- Catalog positions are J2000 / ICRS-like unit-sphere Cartesian coordinates:
  `x = cos δ cos α`, `y = cos δ sin α`, `z = sin δ`.
- Catalog distances are stored in parsecs from HYG's `dist` column. The
  Earth-centred sky dome treats stars as directions at infinity; the external
  galactic viewpoint multiplies the J2000 direction by this distance and
  rotates it into IAU galactic Cartesian coordinates.
- HYG proper motion is converted into a Cartesian tangent vector in radians per
  Julian year.
- Civil input time is UTC.
- Earth rotation and sidereal time use UT1. If DUT1 is unknown, the default is
  zero, which can move sidereal quantities by up to about 13.5 arcsec.
- Ephemeris-facing calculations use TT / approximate TDB as appropriate.
- The local frame is ENU: East, North, Up.
- Azimuth is measured from North toward East.
- Observer latitude is geodetic for solar-system parallax and effectively
  astronomical/geographic for naked-eye stellar projection.
- Catalog and overlay altitudes are geometric. When atmosphere is enabled, the
  renderer applies pressure/temperature-scaled Saemundsson-style refraction to
  stars and Sun/Moon disk directions before projection.

## Render data flow

A typical frame is:

1. Host parses UI / CLI state into observer, time, view, overlays, atmosphere,
   and optional planning settings.
2. Catalog stars are loaded into `catalog::Star` records.
3. Host or `crates/common` converts each star into `renderer::StarInstance`
   with perceptual radius, brightness, colour, and proper-motion fields.
4. Host creates or resizes the `wgpu` surface / target texture.
5. `Camera` combines `Observer`, `LocalView`, aspect ratio, `SkyProjection`,
   `SkyViewpoint`, correction terms, solar-system apparent directions, and
   atmosphere settings into GPU uniforms.
6. `Renderer::render` draws skyglow / bodies / stars into an HDR target,
   tonemaps to the output view, then composites overlay lines and labels in
   LDR screen space.
7. Host presents the surface or copies the headless texture to an image file.

## Renderer pipeline responsibilities

The exact pass layout can change, but responsibilities should stay separated:

- **Camera/uniform preparation**: CPU-side apparent-date, observer-dependent,
  and projection data that the GPU needs for a frame.
- **Skyglow / atmosphere**: diffuse night sky, zodiacal light, airglow, dust,
  sunlit scattering, twilight, moonlit sky, and solar-system disks. Perspective
  reconstructs rays through the inverse view-projection matrix; all-sky modes
  invert the selected Mollweide / Aitoff / Hammer map before rotating the ray
  back to equatorial coordinates. In `SkyViewpoint::GalacticNorth`, this pass
  instead ray-marches the top-down galactic plane intersection and draws a
  compact analytic Milky Way disc for external context.
- **Star pass**: per-star proper motion, corrections, refraction, extinction,
  projection, PSF/glare, and HDR accumulation. In the external galactic
  viewpoint, atmospheric effects are skipped and stars are projected as parsec
  positions in the IAU galactic frame.
- **Overlay pass**: reference circles, grids, constellation lines, boundaries,
  projection, and LDR text labels from the shared bitmap font atlas /
  label-placement pass.
- **Tonemap pass**: local adaptation, mesopic / scotopic split, and conversion
  from HDR radiance-like values to display output.

## Adding a new host

### 1. Choose catalog backend

Native hosts usually use filesystem loading:

```toml
catalog = { path = "../../crates/catalog", features = ["filesystem"] }
```

WASM or single-binary hosts usually use embedded loading:

```toml
catalog = { path = "../../crates/catalog", default-features = false, features = ["embedded"] }
```

Pick one. Avoid making a host depend on runtime files if its deployment target
cannot reliably read them.

### 2. Build observer and view state

```rust
use astronomy::{julian_date_from_unix_seconds, Observer};
use renderer::{Camera, LocalView, SkyProjection, SkyViewpoint};

let jd = julian_date_from_unix_seconds(unix_seconds);
let observer = Observer::from_degrees(35.68, 139.69, jd);
let view = LocalView {
    azimuth_rad: 180_f32.to_radians(),
    altitude_rad: 30_f32.to_radians(),
    fov_y_rad: 70_f32.to_radians(),
};
let mut camera = Camera::new(observer, view, width as f32 / height as f32);
camera.projection = SkyProjection::Perspective; // or Mollweide / Aitoff / Hammer
camera.viewpoint = SkyViewpoint::Earth; // or GalacticNorth for a top-down Milky Way map
```

Interactive hosts should refresh time every frame, or expose an explicit pause /
speed control.

### 3. Convert catalog stars to renderer instances

```rust
use renderer::{build_star_instance, NAKED_EYE_LIMITING_MAGNITUDE};

let stars = catalog::load_from_file("crates/catalog/data/hyg_v42.csv")?;
let limiting_magnitude = NAKED_EYE_LIMITING_MAGNITUDE + 1.5;
let instances = stars
    .iter()
    .map(|s| {
        build_star_instance(
            s.position.into(),
            s.proper_motion.into(),
            s.color,
            s.magnitude,
            limiting_magnitude,
            s.distance_pc,
        )
    })
    .collect::<Vec<_>>();
```

Native hosts should prefer `stars_host_common::load_star_instances_from_file` so
CLI and viewer behaviour cannot drift.

### 4. Create renderer and render loop

```rust
use renderer::Renderer;

let mut renderer = Renderer::new(&device, surface_format, width, height, &instances);

// Per frame:
renderer.update_camera(&queue, &camera, width, height);
let mut encoder = device.create_command_encoder(&Default::default());
renderer.render(&mut encoder, &target_view);
queue.submit([encoder.finish()]);
```

The host remains responsible for surface acquisition, presentation, headless
copyback, resize events, and device-loss handling.

## Build and CI contract

The `Makefile` is the canonical local command surface:

```bash
make setup     # build web package, install frontend deps, download catalog
make cli       # render one PNG; override ARGS="..."
make viewer    # run native desktop viewer
make web       # build WASM and start Vite dev server
make ci        # fmt --check, clippy, tests, wasm check, frontend typecheck
```

When adding a host:

1. Add it to the workspace if it builds for the native target.
2. Exclude it from the workspace if it only builds for `wasm32-unknown-unknown`.
3. Add a `make <host>` target.
4. Add CI coverage for the host's intended target.
5. Update this file if the integration recipe changes.

## Quick API map

```rust
// astronomy
Observer::from_degrees(lat_deg, lng_deg, jd) -> Observer
julian_date_from_unix_seconds(secs) -> f64
TimeScales::from_unix_seconds(secs) -> TimeScales
lmst_radians(jd_ut1, longitude_east_rad) -> f64
equatorial_to_horizontal(ra, dec, lst, lat) -> AltAz
precession_nutation_matrix(...)
refracted_altitude_saemundsson(...)
apparent_sun_topocentric(...)
apparent_moon_topocentric(...)
apparent_planets_topocentric(...)
evening_plan(...)

// catalog
load_from_csv(&str) -> Vec<Star>
load_from_file(path) -> io::Result<Vec<Star>>   // feature = "filesystem"
load_embedded() -> Vec<Star>                    // feature = "embedded"
bv_to_rgb(bv) -> [f32; 3]
radec_hours_deg_to_cartesian(ra_hours, dec_degrees) -> Vec3

// renderer
Renderer::new(&device, format, width, height, &[StarInstance]) -> Renderer
Renderer::resize(&device, width, height)
Renderer::set_overlays(&device, &OverlayConfig)
Renderer::update_camera(&queue, &Camera, width, height)
Renderer::render(&mut encoder, &TextureView)
Camera::new(observer, LocalView, aspect) -> Camera
Camera::rotate_view(daz_rad, dalt_rad)
Camera::zoom_fov(factor)
SkyProjection::{Perspective, Mollweide, Aitoff, Hammer}
OverlayConfig { layers, grid_step_deg, opacity }
```
