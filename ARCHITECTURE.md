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
          ┌──────────────┬──────────────┬──────────────┐
          ▼              ▼              ▼              ▼
      apps/cli       apps/viewer     apps/server      apps/web
      PNG output     native window   HTTP / PNG       WASM + canvas

                  bindings/python  (read-only PyO3 wrapper, L-21)
                  ▲
                  └── calls astronomy + catalog directly; no renderer.
```

`crates/common` is intentionally outside the engine tier even though it lives
under `crates/`. It contains native-host glue: `clap` mirrors of renderer enums,
`chrono` time parsing, schema-versioned JSON session conversion, deterministic
scene preset construction, atmosphere / overlay argument mapping, the shared
catalog-to-renderer conversion for CLI / desktop / server, and — since `L-22`
— the shared headless GPU render pipeline (`crates::common::render`) that
the CLI and the HTTP server both call so the two hosts cannot drift on
device initialisation, readback alignment, or PNG encoding. The web host
bypasses it so WASM stays free of native-only dependencies.

## Crate boundaries

### `crates/astronomy`

Owns scientific and geometric models:

- civil and dynamical time scales: UTC, UT1, TAI, TT, approximate TDB;
- `Observer` and local sidereal-time helpers;
- equatorial-to-horizontal transforms;
- proper-motion epoch math, annual aberration, IAU 2006 precession, compact
  IAU-2000-style nutation, and atmospheric refraction;
- Sun, Moon, planet, Saturn-ring, Galilean-moon, and Titan apparent /
  topocentric helpers (`ephemeris.rs`, `moons.rs`). The default source is the
  `astro` crate's truncated VSOP87 / ELP2000 series (visual tier);
- a DAF/SPK Chebyshev kernel reader (`spk.rs`, `astronomy::SpkKernel`, `L-06`):
  parses NAIF DAF/SPK Type 2 / Type 3 segments from real JPL DE440 kernels and
  evaluates barycentric / geocentric states with body-id center chaining. It
  is dependency-free and feeds no host yet (no kernel is committed; default and
  WASM keep the VSOP87 / ELP2000 fallback) — see ROADMAP `L-06`;
- artificial-satellite TLE parsing + SGP4 propagation, TEME→topocentric
  reduction, conical Earth-shadow visibility, and amateur-grade apparent
  magnitude (`satellites.rs`, V-55; SGP4 via the vendored `sgp4` crate).
  The TEME position and the WGS84 observer position share the one
  GMST-defined inertial frame (`observer_equatorial_position_km`), so the
  satellite reduction reuses the same equatorial → horizontal path as the
  Sun / Moon / planets;
- photometry, illuminants, atmosphere, twilight, and skyglow reference models,
  including the V-39 light-pollution model and the V-39-Atlas `FalchiAtlas`
  parser + bilinear sampler for a compact `FALATL01` zenith-brightness grid
  (`light_pollution_atlas.rs`, IO-free; the host crate does the file read);
- the `V-46` galactic structural model (`galaxy.rs`): a Drimmel & Spergel 2001
  style `milky_way_luminosity_density(x, y, z)` (thin + thick disk, triaxial
  bar, Reid 2019 four-arm log-spiral enhancement) and a double-exponential
  `dust_extinction_az(distance, l, b)` screen, in galactocentric IAU parsecs.
  These are the reference for the external-viewpoint shader, whose WGSL
  constants mirror this module value-for-value (pinned by the `galaxy` tests);
- the V-48 aurora model (`aurora.rs`): the Feldstein-Starkov 1967 auroral-
  oval boundary as a function of Kp, centered-dipole corrected geomagnetic
  latitude / pole bearing (IGRF-13 2020 pole), the apparent elevation of an
  elevated emitting layer, and the statistically-expected apparent arc
  (`aurora_view`) the renderer paints. The renderer carries an `AuroraLayer`
  and packs `aurora_geometry` / `aurora_params` rows appended at the end of
  `CameraUniform`; `shaders/skyglow.wgsl::aurora_radiance` composites the
  green / red / magenta emission near the horizon;
- comet osculating-element two-body propagation (elliptical / parabolic /
  hyperbolic), J2000-equatorial geocentric reduction sharing the Earth's
  VSOP87D position, Bobrovnikoff-Bowell coma photometry, and anti-solar ion /
  β = 0.6 dust-syndyne tail directions (`comets.rs`, V-49);
- planning helpers such as rise / transit / set, twilight bands, and the
  `L-09` observation-planning layer: Krisciunas-Schaefer 1991 Moon-impact
  (`moon_impact`), visibility scoring (`visibility_score`), recommended-
  object ranking (`rank_targets`), and RFC 5545 iCalendar export
  (`icalendar_for_targets`). The Moon-free baseline reads the `V-39`
  light-pollution zenith brightness, so scores follow the configured site;
  the CLI (`--plan-json` / `--plan-ical`) and web (`planning_recommended_json`
  / `planning_ical`) hosts consume these helpers without re-deriving them.

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
- large-catalog ingest (`L-17`) in `ingest.rs`: `HipparcosCsvBackend`,
  `Tycho2CsvBackend`, and `GaiaDr3CsvBackend` parse normalised VizieR / Gaia
  CSV exports by column name behind the same `CatalogBackend` trait, preserve
  native + cross identifiers, derive Tycho V/B−V from VT/BT (ESA 1997), and
  page through `CatalogQuery`/`CatalogPage`. The raw archives are fetched on
  demand (`scripts/fetch-{hipparcos,tycho2,gaia-dr3-subset}.sh`, manifest
  `runtime-service` rows) and never committed; a committed HIP↔HD bright-star
  anchor index (`bright_star_xmatch.csv`, generated from HYG) backs the
  identifier round-trip tests. Gaia LOD streaming and host wiring are the
  `L-17` follow-up;
- CPU-side `CatalogIdentifiers` for HYG / HIP / HD / Tycho-2 / Gaia DR3,
  populated by the HYG and large-catalog backends;
- the `DeepSkyCatalog` trait + embedded `MessierCatalog` and
  `NgcBrightCatalog` implementations consumed by the renderer's deep-sky
  overlay (`V-42`); the trait is the slot for the planned runtime
  full-OpenNGC streaming backend;
- visual double / binary resolution (`V-54`): `doubles::resolve_doubles`
  runs inside both load paths (`load_from_csv` and the embedded
  `load_from_binary`) and replaces each merged HYG primary that matches
  the WDS bootstrap table (`double_stars.csv`) with two component `Star`s
  at the catalog separation / position angle. Because it sits in the
  catalog layer, every host gets the split with no host-specific code;
- B−V colour conversion and RA/Dec-to-Cartesian helpers.

Catalog stars are renderer-independent. They should not know about `wgpu`,
window size, camera state, or UI labels. Large-catalog storage, paging, and LOD
rules are specified in [`docs/catalog-backend-design.md`](docs/catalog-backend-design.md).

### `crates/renderer`

Owns GPU-facing rendering state:

- `Renderer` lifecycle and render passes;
- `Camera`, `LocalView`, `SkyProjection`, `SkyViewpoint`, `EyepieceSimulation`,
  `OpticalDesign` (V-45 telescope-side optics), and GPU camera uniforms;
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
- the `L-23` guided-tour content (`tour.rs`): `Tour` / `TourStep` plus a
  declarative, host-agnostic `TourScene` whose `to_session_scene` reuses the
  preset `earth_scene` builder, and the built-in `first_night_tour()`. The
  renderer knows nothing about tours; hosts present captions natively (CLI
  flags, the viewer `T` key, and the web `TourPanel`, the latter mirroring
  the content in `apps/web/frontend/src/tour.ts` because the web renderer
  does not depend on this crate);
- filesystem catalog loading plus conversion to renderer `StarInstance`s;
- the V-39-Atlas light-pollution resolver: `load_falchi_atlas` reads the
  optional Falchi 2016 grid named by `STARS_FALCHI_ATLAS`, and
  `resolve_light_pollution` maps a `LightPollution::Atlas2016 { lat, lng }`
  scene value to a sampled `LightPollution::Sqm` at render time (the session
  keeps the `Atlas2016` lat/lng). File IO lives here, not in `astronomy`.

Do not put core astronomical or rendering logic here.

### `apps/*`

Hosts own platform lifecycle:

- `apps/cli`: create a headless texture, optionally load/write a JSON session
  or built-in preset, render once, copy padded rows back to CPU memory, and
  write PNG;
- `apps/server`: HTTP host (axum). Routes scene JSON to the shared
  `stars_host_common::render::render_scene_from_catalog_path` pipeline
  and streams the PNG bytes back. Endpoints: `GET /healthz`, `GET
  /presets[/{id}]`, `POST /render?width=&height=&skyglow=`. No engine
  or render logic of its own.
- `apps/viewer`: optionally load a JSON session or built-in preset, manage a `winit` event loop,
  surface resize, input, and frame pacing;
- `apps/web`: expose a WASM `StarView`, keep JS/UI state, load/copy/download
  JSON sessions, resize the canvas, and call into the shared renderer. The
  V-56 object-search surface (`lookup_object`, `goto_object`) is part of the
  same WASM facade: the search index lives in `crates/catalog::search`, so
  every host gets the same ranking by calling `catalog::search(query, n)`;
  `goto_object` resolves the returned `SearchId` back to a topocentric
  apparent `(alt, az)` via the existing `apparent_*_topocentric` paths so
  the info panel sees the same ephemeris the renderer does. The same
  facade emits the `L-19` SIMBAD / VizieR deep links (`simbadUrl` /
  `vizierUrl`): the pure URL builders live in `catalog::links`
  (`simbad_query_url` / `vizier_query_url` over `StarIdentifiers`) so the
  WASM binding and the native hosts share one source of truth, re-exported
  on the `stars_host_common` path for CLI / viewer. These are inert
  strings — no host or the renderer ever calls the network, keeping
  deterministic renders offline. The `L-24` accessibility layer is part of
  this host only: global a11y CSS in `index.html` (`:focus-visible`,
  `.sr-only`, reduced-motion / forced-colors), a keyboard-operable
  `StarCanvas` (arrow-key pan, `+`/`-` zoom), modal focus management +
  focus trap on the popovers, and the WAI-ARIA tabs pattern on the settings
  panel. It is attribute-level only — no session-schema or renderer change —
  so the renderer CVD-safe overlay palette and audio cues remain `L-24`
  follow-ups.

### `bindings/python` (L-21, read-only)

PyO3 wrapper around the **read-only** `astronomy` + `catalog` public
surface. Built as a `cdylib + rlib` (`stars-py`) and loaded from a
Python interpreter via `maturin develop --features extension-module`;
`cargo check -p stars-py` is the plain `make ci` gate via
`make pyo3-check`. The binding sits **outside** the engine tier: it
calls `Observer`, `apparent_sun_moon`, `apparent_planets`,
`apparent_galilean_moons`, `apparent_titan`, `StarCatalog`, the
observation-planning surface (`evening_plan`, `rise_transit_set`,
`twilight_*`), the occultation / eclipse finders (`active_occluders`,
`find_lunar_occultation`, `find_solar_eclipse`, `find_planet_transit`,
`find_mutual_planetary_occultation`), and a `serde_json`-only `Session`
round-trip of the `crates/common` JSON schema, plus `.altaz(observer)`
projections — but does not pull in `renderer`, `common`, or any host
crate. Notebook reviewers can therefore reproduce the exact apparent
positions, magnitudes, planning windows, and eclipse circumstances the
renderer consumes without dragging in WGPU. The only out-of-scope
follow-up is a maturin wheel-matrix CI job (needs a Python toolchain in
CI); the API surface itself is complete.

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
`schemaVersion: 6` (v2 unified the spectral extinction state via `V-37`; v3
added `surfaceAlbedo` for the Hošek-Wilkie daylight model in `V-38`; v4 added
the scintillation block for `V-24`; v6 added the `outputColourspace` field for
`V-50` output colour management). The
native representation lives in
`stars_host_common::session`; the web frontend keeps a TypeScript mirror in
`apps/web/frontend/src/session.ts`. Field names are stable host-facing names
(camel-case objects and kebab-case enum values), with degrees for UI-facing
angles and explicit time-scale fields (`jdUtc`, `jdUt1`, `jdTai`, `jdTt`,
`jdTdb`, `taiMinusUtcSeconds`, `dut1Seconds`).

A session records observer, view, overlays, projection/viewpoint, custom
external viewpoint vectors, atmosphere/refraction controls, planet visibility,
eyepiece optics, the output colour space (`V-50`: `srgb` / `display-p3` /
`rec2020`), active correction flags, catalog snapshot metadata, and app
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
  sunlit scattering, twilight, moonlit sky, and solar-system disks. Airglow
  is decomposed into three emission systems (O I 557.7 nm green line,
  Na D 589 nm, OH Meinel red/IR; V-28); each carries its own Van Rhijn
  layer-altitude correction and a Rec.709-luminance-preserving sRGB tint
  vector, summed per channel before extinction. The Rust reference lives
  in `astronomy::skyglow::{airglow_components, airglow_rgb_s10}` and the
  shader port in `shaders/skyglow.wgsl::airglow_radiance_rgb`. Sun and
  Moon (and, via V-51b, planet) disks share a single analytic-mask occluder
  array (`CameraUniform::occluders`, `MAX_OCCLUDERS = 16`) populated each
  frame from `astronomy::active_occluders(observer)` and mapped through the
  same `apparent_disk_direction_j2000` pipeline as the bare Sun and Moon
  directions — so any front-disk on back-disk pair (solar eclipse, planetary
  transit, mutual planetary occultation, Galilean shadow transit on
  Jupiter) is one shader uniform write away with no depth or stencil
  attachments. Artificial satellites (V-55) ride the same skyglow pass:
  `Camera::satellite_uniforms` propagates the `SatelliteLayer`'s TLEs with
  SGP4 every frame (LEO satellites sweep the sky in seconds, so unlike the
  cached VSOP87 planet block this is recomputed per frame), maps each
  satellite direction through the same `apparent_disk_direction_j2000`
  pipeline as the planets, and packs direction+magnitude, a streak endpoint
  (the direction one exposure later), and an above-horizon-and-sunlit
  visibility flag into a satellite uniform block; `satellite_radiance` draws
  each visible satellite as a neutral point sprite (or a great-circle motion
  streak when the exposure field is positive). Meteor showers (V-47) ride the
  same pass: `Camera::meteor_uniforms` asks `astronomy::meteor_stream` for a
  deterministic Poisson sample of shower + sporadic meteors (the rate from the
  Koschack-Rendtel 1990 observed-rate formula and the Jenniskens 1994
  solar-longitude activity profile, seeded by `(seed, jd_utc/window)`), maps
  each streak's head/tail through the same `apparent_disk_direction_j2000`
  pipeline, and packs them into a `meteor_segments` block appended at the END
  of `CameraUniform`; the self-contained `meteor_radiance` evaluator reuses the
  great-circle `satellite_streak_mask` from a single composition insertion
  point. Comets (V-49) ride the same skyglow pass: `Camera::comet_uniforms`
  propagates the `CometLayer`'s osculating elements two-body every frame, maps
  each nucleus through the same `apparent_disk_direction_j2000` pipeline, and
  packs nucleus direction+magnitude, coma radius, and ion (anti-solar) / dust
  (β = 0.6 syndyne) tail-tip sky directions into an appended comet uniform
  block; `comet_radiance` draws a soft 1/ρ coma plus great-circle ion / dust
  tail streaks. The V-52d Galilean-shadow producer

  (`astronomy::galilean_shadow_disks_at`) feeds the same uniform; it
  also drives a CPU-side "moon behind Jupiter" cull on the V-52b
  Galilean-moon sprite path via a negative-radius sentinel in
  `CameraUniform::galilean_eq_radius[i].w`. The lunar disk fragment composes a
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
  The twilight composition (V-33) is zenith-symmetric in luminance and
  picks up its anti-solar structure inside the same pass via V-27: the
  shader evaluates `antitwilight_arch_multiplier(sun_alt, relative_az,
  view_alt)` and `earth_shadow_band_multiplier(...)` (matching the Rust
  fits in `astronomy::atmosphere`) and multiplies the per-channel
  twilight radiance by both, so the Belt of Venus and Earth-shadow band
  appear only in the anti-solar half-sky during civil twilight.
  Perspective
  reconstructs rays through the inverse view-projection matrix; all-sky modes
  invert the selected Mollweide / Aitoff / Hammer map before rotating the ray
  back to equatorial coordinates. In external viewpoints, this pass instead
  emission-absorption ray-marches the `V-46` galactic structural model from the
  configured parsec-scale origin: `gal_density` (thin/thick disk, triaxial bar,
  Reid 2019 spiral arms) attenuated by the `gal_dust_density` dust disk, so the
  bar, arms, and dark dust lanes resolve. Its constants mirror
  `astronomy::galaxy` and are guarded by that module's pinned tests.
- **Star pass**: per-star proper motion, corrections, refraction, extinction,
  projection, PSF/glare, and HDR accumulation. In the external galactic
  viewpoint, atmospheric effects are skipped and stars are projected as parsec
  positions in the IAU galactic frame. Refraction is wavelength-dependent
  (`V-25`): the vertex stage projects three apparent altitudes per star
  using Edlén 1966 dispersion at R = 620 nm, G = 550 nm, B = 440 nm, and
  emits green-relative pixel offsets so the fragment stage can sample the
  radial Spencer PSF at three centres and bake the chromatic streak into
  the footprint rather than adding it as a post-process tint. The Sun and
  Moon disk masks in the skyglow pass apply the same per-channel offsets
  to reproduce the red lower limb / blue upper limb of a horizon-grazing
  Sun or Moon. When the eyepiece simulation is active in a perspective
  Earth view (`V-45`), the fragment stage composites a telescope
  *instrument* PSF on top of the Spencer eye PSF: an obstructed-aperture
  Airy pattern (`2J1(x)/x`, annular for the central obstruction), spider
  diffraction spikes that roll with the OTA, a per-channel chromatic ring
  shift for achromats, and an exit-pupil cos⁴ vignette. The instrument
  parameters (Airy radius in pixels, obstruction ratio, vane count, spike
  angle, chromatic fraction, vignette) ride two `instrument_optics`
  `CameraUniform` rows appended at the end of the struct and are zero
  outside eyepiece mode, so the naked-eye PSF is unchanged. The pixel-level
  Airy radius scales with magnification, so the diffraction pattern only
  resolves at high power, as in a real eyepiece.
- **Overlay pass**: reference circles, grids, constellation lines, boundaries,
  projection, and LDR text labels from the shared bitmap font atlas /
  label-placement pass.
- **Tonemap pass**: local adaptation, mesopic / scotopic split, conversion
  from HDR radiance-like values to display output, and the `V-50` output
  colour-management step — a linear sRGB→target gamut matrix
  (`renderer::colourspace`) applied after the Reinhard operator and uploaded
  via a dedicated tonemap uniform. The swap-chain / PNG keeps the sRGB
  transfer function; only the primaries change and are then tagged on the
  output (PNG `cHRM`, sRGB-fallback canvas).

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
