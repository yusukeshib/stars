# Architecture

This document explains how the `stars` engine is split, how data flows through
it, and how a new host should integrate the renderer. It replaces the old
`USAGE.md`: this is not just user-facing usage, but the architectural contract
between engine crates and host applications.

## High-level shape

```txt
┌─────────────────────────────┐      ┌─────────────────────────────┐
│ crates/astronomy            │      │ crates/catalog              │
│  time scales                │      │  CatalogBackend trait       │
│  observer / local frame     │      │  HYG CSV / embedded catalog │
│  corrections                │      │  Star records + identifiers │
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
`chrono` time parsing, schema-versioned JSON session conversion, deterministic
scene preset construction, atmosphere / overlay argument mapping, and shared
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
- Sun, Moon, planet, Saturn-ring, and Galilean-moon apparent /
  topocentric helpers (`ephemeris.rs`, `moons.rs`);
- photometry, illuminants, atmosphere, twilight, and skyglow reference models;
- planning helpers such as rise / transit / set and twilight bands.

This crate should not depend on a renderer, a UI framework, or host-specific
argument parsing.

### `crates/catalog`

Owns star catalog ingestion, deep-sky catalogues, and catalog-space conversions:

- `CatalogBackend`, `CatalogQuery`, `CatalogPage`, and `CatalogSource` as the
  Library-track backend seam (`L-17`) for larger catalogs;
- `HygCsvBackend` for filesystem-backed HYG loading;
- `load_from_csv(&str)` and `load_from_file(path)` compatibility helpers;
- `load_embedded()` behind the `embedded` feature;
- HYG row filtering and conversion into `Star` records;
- CPU-side `CatalogIdentifiers` for HYG / HIP / HD now, with Tycho-2 / Gaia DR3
  slots reserved for later ingest;
- the `DeepSkyCatalog` trait + embedded `MessierCatalog` and
  `NgcBrightCatalog` implementations consumed by the renderer's deep-sky
  overlay (`V-42`); the trait is the slot for the planned runtime
  full-OpenNGC streaming backend;
- B−V colour conversion and RA/Dec-to-Cartesian helpers.

Catalog stars are renderer-independent. They should not know about `wgpu`,
window size, camera state, or UI labels. Large-catalog storage, paging, and LOD
rules are specified in [`docs/catalog-backend-design.md`](docs/catalog-backend-design.md).

### `crates/renderer`

Owns GPU-facing rendering state:

- `Renderer` lifecycle and render passes;
- `Camera`, `LocalView`, `SkyProjection`, `SkyViewpoint`, `EyepieceSimulation`,
  and GPU camera uniforms;
- `StarInstance` and `build_star_instance`;
- overlay geometry, text labels, and `OverlayKind` / `OverlayConfig`;
- HDR target, skyglow pass, tonemap pass, star shader, text shader, and atmosphere uniforms.

The renderer is platform-agnostic. It expects the host to provide a `wgpu::Device`,
queue, target format, target `TextureView`, and resize / surface management.

### `crates/common`

Owns native-host convenience only:

- CLI-facing mirrors of renderer enums;
- native atmosphere / overlay / eyepiece argument conversion;
- RFC3339 / now time parsing;
- schema-versioned JSON session load/save and conversion into native render state;
- deterministic named scene presets that compile to normal sessions;
- filesystem catalog loading plus conversion to renderer `StarInstance`s.

Do not put core astronomical or rendering logic here.

### `apps/*`

Hosts own platform lifecycle:

- `apps/cli`: create a headless texture, optionally load/write a JSON session
  or built-in preset, render once, copy padded rows back to CPU memory, and
  write PNG;
- `apps/viewer`: optionally load a JSON session or built-in preset, manage a `winit` event loop,
  surface resize, input, and frame pacing;
- `apps/web`: expose a WASM `StarView`, keep JS/UI state, load/copy/download
  JSON sessions, resize the canvas, and call into the shared renderer.

## Coordinate and time conventions

All crates should preserve these conventions:

- Angles are radians internally unless a helper explicitly says degrees or
  hours.
- Catalog positions are J2000 / ICRS-like unit-sphere Cartesian coordinates:
  `x = cos δ cos α`, `y = cos δ sin α`, `z = sin δ`.
- Catalog distances are stored in parsecs from HYG's `dist` column. The
  Earth-centred sky dome treats stars as directions at infinity; external
  viewpoints multiply the J2000 direction by this distance and rotate it into
  IAU galactic Cartesian coordinates. In that frame the Sun is `(0, 0, 0)`,
  `+X` points to galactic longitude `l=0°`, `+Y` to `l=90°`, and `+Z` to the
  north galactic pole. `SkyViewpoint::GalacticNorth` is the preset origin
  `(0, 0, 30000)` pc looking at the Sun with up `(0, 1, 0)`;
  `SkyViewpoint::CustomExternal` uses the host-provided origin, target, and up
  vectors in the same frame.
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

## Session files

Portable scene state uses schema-versioned JSON with the current top-level
`schemaVersion: 3` (v2 unified the spectral extinction state via `V-37`; v3
added `surfaceAlbedo` for the Hošek-Wilkie daylight model in `V-38`). The
native representation lives in
`stars_host_common::session`; the web frontend keeps a TypeScript mirror in
`apps/web/frontend/src/session.ts`. Field names are stable host-facing names
(camel-case objects and kebab-case enum values), with degrees for UI-facing
angles and explicit time-scale fields (`jdUtc`, `jdUt1`, `jdTai`, `jdTt`,
`jdTdb`, `taiMinusUtcSeconds`, `dut1Seconds`).

A session records observer, view, overlays, projection/viewpoint, custom
external viewpoint vectors, atmosphere/refraction controls, planet visibility,
eyepiece optics, active correction flags, catalog snapshot metadata, and app
version. Hosts should reject unknown future schema versions instead of silently
interpreting them. Compact web URL query parameters remain a convenience format;
JSON sessions are the reproducibility format intended for presets, validation
scenes, and cross-host exchange.

## Render data flow

A typical frame is:

1. Host parses UI / CLI state into observer, time, view, overlays, atmosphere,
   optional telescope eyepiece optics, and optional planning settings.
2. Catalog stars are loaded through a catalog backend into `catalog::Star`
   records. The current native path uses `HygCsvBackend`; the web path uses the
   compact embedded HYG artifact.
3. Host or `crates/common` converts each star into `renderer::StarInstance`
   with perceptual radius, brightness, colour, and proper-motion fields.
4. Host creates or resizes the `wgpu` surface / target texture.
5. `Camera` combines `Observer`, `LocalView`, aspect ratio, `SkyProjection`,
   `SkyViewpoint`, optional `ExternalViewpoint`, optional `EyepieceSimulation`,
   correction terms, solar-system apparent directions, and atmosphere settings
   into GPU uniforms. When eyepiece mode is enabled for an Earth-centred
   perspective view, the camera uses the derived true field of view instead of
   the free FoV slider.
6. `Renderer::render` draws skyglow / bodies / stars into an HDR target,
   tonemaps to the output view, then composites overlay lines and labels in
   LDR screen space.
7. Host presents the surface or copies the headless texture to an image file.

## Portable sessions and scene presets

Schema-versioned JSON sessions describe reproducible inputs, not host UI
implementation details. Version 1 captures:

- observer latitude / longitude;
- UTC plus derived UT1 / TAI / TT / approximate TDB metadata and DUT1;
- view direction, field of view, projection, and viewpoint;
- overlay and label configuration;
- active corrections and atmosphere / refraction settings, including observer
  altitude in the atmosphere model;
- eyepiece optics;
- catalog backend identity, filtering, and data snapshot / manifest reference;
- app and schema version.

Scene presets are named sessions checked into the repository or generated by a
script. They are the source for documentation screenshots, validation-gallery
renders, demo links, and notebook examples. Hosts may add UI-only state around a
session, but should keep the core schema stable and round-trippable.

## Catalog backend scaling plan

HYG is small enough to load eagerly and embed for WASM. Hipparcos, Tycho-2,
Gaia, and dense deep-sky catalogs need a design step before ingestion. The
planned backend should define:

- a renderer-independent catalog trait / query shape;
- stable identifier mapping for HIP, HD, TYC, Gaia `source_id`, Messier, NGC,
  IC, and AAVSO-style identifiers;
- magnitude, distance, colour, and quality filters;
- spatial indexing or tiling for interactive level-of-detail;
- a small deterministic embedded subset for WASM and examples;
- optional streaming / paging for native or server hosts;
- manifest references so sessions and validation renders can name the exact
  data snapshot they used. The manifest lives at `data/manifest.toml` and is
  parsed by `stars-manifest`; backends cite artifacts by their stable `id`
  (e.g. `hyg-v4.2`) so a session JSON can pin which snapshot a render used.

Do not make `crates/renderer` depend on a large-catalog storage format. It
should continue to receive compact render instances and metadata prepared by the
host or catalog layer.

## Renderer pipeline responsibilities

The exact pass layout can change, but responsibilities should stay separated:

- **Camera/uniform preparation**: CPU-side apparent-date, observer-dependent,
  eyepiece true-field, and projection data that the GPU needs for a frame.
- **Skyglow / atmosphere**: diffuse night sky, zodiacal light, airglow, dust,
  sunlit scattering, twilight, moonlit sky, and solar-system disks. Sun and
  Moon (and, via V-51b, planet) disks share a single analytic-mask occluder
  array (`CameraUniform::occluders`, `MAX_OCCLUDERS = 16`) populated each
  frame from `astronomy::active_occluders(observer)` and mapped through the
  same `apparent_disk_direction_j2000` pipeline as the bare Sun and Moon
  directions — so any front-disk on back-disk pair (solar eclipse, planetary
  transit, mutual planetary occultation) is one shader uniform write away
  with no depth or stencil attachments. The lunar disk fragment composes a
  Lambertian lit-side term and a Lambertian dark-side earthshine term
  additively (V-26, "Da Vinci glow"); the dark-side luminance is the
  closed-form Goode/Danjon anchor in
  `astronomy::illuminants::earthshine_disk_luminance_cd_m2`, and the
  dark side is attenuated by the same per-channel Schaefer 1993 /
  Kasten-Young 1989 extinction the diffuse sky pass uses (V-37). Stellar
  extinction and daylight scattering both read the same canonical
  (β, α, DU, observer altitude) optical-depth state through
  `astronomy::atmosphere::extinction_coefficients` (V-37); the renderer
  derives an effective Linke turbidity from β at uniform-build time so
  the daylight scattering term and the stellar k(λ) reddening can't
  disagree. The daylight sky-dome radiance comes from the Hošek-Wilkie
  2012 analytic model (V-38), cooked per frame on the CPU in
  `astronomy::atmosphere::hosek_wilkie::cook` and evaluated on the GPU.
  Perspective
  reconstructs rays through the inverse view-projection matrix; all-sky modes
  invert the selected Mollweide / Aitoff / Hammer map before rotating the ray
  back to equatorial coordinates. In external viewpoints, this pass instead
  ray-marches the galactic-plane intersection from the configured parsec-scale
  origin and draws a compact analytic Milky Way disc for context.
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
camera.viewpoint = SkyViewpoint::Earth; // or GalacticNorth / CustomExternal
camera.external_viewpoint = renderer::ExternalViewpoint::new(
    [8_200.0, 0.0, 2_000.0], // origin_pc in IAU galactic Cartesian parsecs
    [0.0, 0.0, 0.0],         // target_pc
    [0.0, 0.0, 1.0],         // up vector in the same frame
);
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
CatalogBackend::load(CatalogQuery) -> Result<CatalogPage, CatalogError>
CatalogSource::{HYG_CSV, HYG_EMBEDDED}
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
