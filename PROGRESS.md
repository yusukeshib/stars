# Progress

This document is the implementation log for `stars`. It records what has been
implemented well enough to count as shipped. The forward-looking plan lives in
[`ROADMAP.md`](ROADMAP.md).

A feature belongs here when it is implemented, documented in the relevant code
or docs, covered by tests where numerical output matters, and wired into the
relevant host applications.

## Summary

Work is organised along two orthogonal tracks (see [`ROADMAP.md`](ROADMAP.md)):
**V — Visual** and **L — Library / platform**.

Shipped:

- **Visual track** — identification overlays and text labels (`V-01`–`V-12`),
  physical dark-sky visual pipeline (`V-13`–`V-23`), atmospheric refraction
  and Sun / Moon / planet rendering (`V-29`–`V-36`), unified spectral
  extinction — one (β, α, DU) state shared by the stellar and daylight
  paths (`V-37`), the Hošek-Wilkie 2012 daylight sky-dome model as the
  daylight scattering source (`V-38`, replacing the legacy Preetham 1999
  path outright), naked-eye atmospheric scintillation (`V-24`,
  Young 1967 / Dravins 1997-98 with a deterministic UT1-keyed noise
  path outright), full-sky projections (`V-40`), out-of-Earth galactic
  and custom external viewpoints (`V-41`, `V-44`),
  telescope eyepiece simulation (`V-43`), the deep-sky overlay with
  Messier objects plus the bright NGC / IC subset (`V-42`), the
  resolved-open-cluster slice for Pleiades / Beehive / Double Cluster
  (`V-53`), and the observer-side Bortle / SQM light-pollution scaling
  of the dark-sky background with sodium / LED tint plus the `V-39-Atlas`
  Falchi 2016 World Atlas loader that samples zenith brightness by observer
  lat/lng (`V-39`), and the statistically-expected aurora display driven by a
  geomagnetic Kp index (`V-48`, Feldstein-Starkov oval + Chamberlain
  emission heights).
- **Library track** — IAU-grade time / precession / nutation / aberration /
  proper motion (`L-01`–`L-05`), planning helpers (`L-07`, `L-08`),
  observation-planning polish — Krisciunas-Schaefer 1991 Moon-impact /
  visibility scoring, recommended-object ranking, favourites, and
  iCalendar export across the astronomy crate, CLI, and web hosts
  (`L-09`),
  schema-versioned JSON sessions (`L-10`, `L-11`), deterministic scene
  presets (`L-12`), notebook reproducibility examples (`L-13`), catalog
  backend scaling scaffold (`L-16`), validation / demo gallery (`L-27`),
  citation metadata (`L-25`), standards-compliance document (`L-26`),
  the data provenance manifest (`L-15`), and SIMBAD / VizieR external
  catalogue deep links from existing identifiers (`L-19`).

Still open:

- **Visual track** — niche visual features (`V-45`–`V-50`), rare
  phenomena (`V-47`–`V-49`). A follow-up PR will add a runtime streaming
  backend for the full ~14,000-entry OpenNGC catalogue on top of the
  embedded `V-42` subset shipped here.
- **Library track** — DE440 ephemerides (`L-06`), large catalog ingest
  (`L-17`), identifier preservation (`L-18`),
  variable-star light curves (`L-20`), Python bindings (`L-21`),
  headless server (`L-22`), guided education (`L-23`), accessibility
  (`L-24`), public demo gallery (`L-14`).

Earlier entries below refer to the legacy Phase 1 / 1' / 2 / 3 / 4 grouping
that shipped before the V / L split landed; the IDs in those entries
(`P1-NN`, `P1P-NN`, `P2-NN`, `P3-NN`, `P4-NN`) have new names in `ROADMAP.md`
and are noted where useful.

## Identification overlays and labels (legacy `Phase 1`)

### Overlay system

Implemented a renderer-side overlay library and host controls for common sky
reference geometry.

Shipped capabilities:

- horizon;
- cardinal markers;
- altitude / azimuth grid;
- equatorial grid;
- ecliptic;
- celestial equator;
- meridian;
- galactic equator;
- constellation lines;
- constellation boundaries.

Primary implementation areas:

- `crates/renderer/src/overlay.rs`
- `crates/renderer/src/constellations.rs`
- `crates/renderer/data/constellation_lines.csv`
- `crates/renderer/data/constellation_boundaries.csv`
- host overlay controls in `apps/cli`, `apps/viewer`, and `apps/web`

### Host overlay controls

Implemented overlay controls across the three reference hosts:

- CLI flags for selecting overlays, disabling overlays, grid step, and opacity.
- Desktop viewer flag parity with the CLI.
- Web settings UI for overlay selection, organized into view/object,
  overlay, planning, atmosphere, and session sections.
- Web overlay controls grouped by reference geometry, constellations, labels,
  and line styling.
- Web localStorage persistence for observer and view state.
- Web location panel address lookup that geocodes a place name into latitude /
  longitude for the observer.

Primary implementation areas:

- `apps/cli`
- `apps/viewer`
- `apps/web/frontend`
- `crates/common`

### Text labels

Implemented a shared label renderer that projects sky positions into
screen-space, applies a simple collision/priority pass, and draws text from a
built-in bitmap font atlas after tone mapping.

Shipped capabilities:

- top-50 bright-star labels with proper names plus Bayer / Flamsteed-style
  designations generated from HYG v4.2;
- Sun, Moon, and Mercury-through-Neptune labels from the renderer apparent-body uniforms;
- constellation-name labels anchored by bright-star centroids;
- default N/E/S/W cardinal labels;
- optional local degree labels for alt-az grids.

Primary implementation areas:

- `crates/renderer/src/text.rs`
- `crates/renderer/src/shaders/text.wgsl`
- `crates/renderer/build.rs`
- host overlay controls in `apps/cli`, `apps/viewer`, and `apps/web`

## Physical dark-sky visual pipeline (legacy `Phase 1'`)

The dark-sky visual realism layer (Visual track, `V-13`–`V-23`) is
implemented. It is orthogonal to positional precision: the goal is that the
sky looks physically plausible to a dark-adapted observer.

### Photometry and colour

Implemented:

- magnitude to physical illuminance scale;
- mesopic chromatic-fidelity weighting;
- scotopic / Purkinje-shifted desaturation for faint stars;
- B−V to effective temperature to blackbody / CIE XYZ / sRGB style catalogue
  colour pipeline;
- perceptual star instance generation for renderer input.

Primary implementation areas:

- `crates/astronomy/src/photometry.rs`
- `crates/catalog/src/color.rs`
- `crates/renderer/src/vertex.rs`

### HDR, glare, and tone reproduction

Implemented:

- HDR render target;
- Spencer-style eye PSF / glare approximation;
- ciliary corona support;
- adaptive Reinhard-style tone reproduction;
- rod/cone separation and local adaptation in the tonemap path.

Primary implementation areas:

- `crates/renderer/src/tonemap.rs`
- `crates/renderer/src/pipeline.rs`
- `crates/renderer/src/renderer.rs`
- `crates/renderer/src/shaders/*` where generated / included by renderer code

### Atmosphere and night-sky background

Implemented:

- Kasten-Young airmass;
- per-channel extinction coefficients, now derived from the **unified
  spectral extinction model** (`V-37`): one canonical (β, α, DU,
  observer altitude) state evaluates Schaefer 1993's
  Rayleigh + Ångström aerosol + Chappuis ozone decomposition at R / G / B
  representative wavelengths, and the same `β` feeds the daylight
  scattering shader's Mie term and the twilight aerosol load — so the
  stellar and daylight paths cannot disagree about how reddened a given
  sky should be. Hardie 1962 mid-quality site is reproduced within 0.03
  mag/airmass at (β=0.10, α=1.3, DU=300); the session schema bumped to
  v2 with the legacy `turbidity` / `visibilityKm` fields removed.
- diffuse sky background fit;
- Milky Way / integrated starlight style contribution;
- zodiacal light;
- gegenschein;
- airglow floor;
- analytic dust extinction;
- Rust reference models and shader-side evaluation.

Primary implementation areas:

- `crates/astronomy/src/skyglow.rs`
- `crates/astronomy/src/photometry.rs`
- `crates/renderer/src/skyglow.rs`

### Hošek-Wilkie 2012 daylight sky model (`V-38`)

Replaced the Preetham 1999 daylight evaluator with the Hošek-Wilkie
2012 analytic full-spectral sky-dome model as the default daylight
radiance source. HW is the published modern replacement for Preetham:
it stays finite and positive below sun_alt = 5° (where Preetham's
asymptotic Perez fit fails), handles high turbidity correctly, and
couples to ground albedo.

Shipped:

- Vendored RGB coefficient table (upstream BSD 3-clause release v1.4a,
  22 Feb 2013): `crates/astronomy/data/hosek_wilkie/coefficients_rgb.bin`,
  regenerated from the upstream `ArHosekSkyModelData_RGB.h` by
  `scripts/build-hosek-wilkie.py` and pinned in `data/manifest.toml`
  under `hosek-wilkie-2012-rgb-v1.4a` (28,816 bytes).
- `crates/astronomy/src/atmosphere/hosek_wilkie.rs`: binary loader
  (validated against the documented magic / size), `cook(turbidity,
  albedo, sun_elev)` (quintic-Bezier elevation blend, linear in
  turbidity + albedo) and `radiance(params, θ, γ)` evaluator returning
  per-channel W·m⁻²·sr⁻¹; the standard 683 lm/W luminous efficacy
  converts to the cd/m² scale the rest of the skyglow pipeline uses.
- `crates/renderer/src/camera.rs`: per-frame CPU cook + nine vec4
  coefficient rows plus a radiance vec4 added to `CameraUniform`; sky
  model selector encoded in `atmosphere_params.w` (0 = off, 1 = HW
  default, 2 = Preetham legacy). `Atmosphere::surface_albedo` and
  `Atmosphere::surface_albedo` added with per-preset defaults
  (clear-rural ≈ 0.10, hazy-urban ≈ 0.13, high-altitude ≈ 0.30).
- `crates/renderer/src/shaders/skyglow.wgsl`: WGSL port of the HW
  evaluator replaces the Preetham daylight path; the legacy WGSL
  `preetham_sky_luminance_rgb`, `perez_distribution`, and
  `xyy_to_linear_rgb` helpers are removed.
- CLI / viewer: `--surface-albedo` flag; `AtmosphereOverrides`
  extended; web frontend exposes the slider in the atmosphere settings
  card with EN / JA labels and persists it through the session JSON
  and localStorage paths.
- `SessionAtmosphere` adds `surfaceAlbedo` as a required field; the
  JSON session schema bumps to v3 since the daylight model is no
  longer a stored choice.
- Turbidity bridge: the V-37 Ångström β → Linke turbidity helper
  keeps a single home but is now exposed under the model-neutral name
  `linke_turbidity_from_aerosol`. HW consumes it directly through
  `astronomy::atmosphere::hosek_wilkie::turbidity_from_aerosol`.

Tests pinned at the boundaries that motivated the upgrade:

- HW radiance is finite and non-negative across the upper hemisphere
  at sun_alt = 1°, T = 4 (Preetham's Perez asymptote goes negative
  here).
- Zenith luminance is monotone in turbidity and in ground albedo, and
  the per-channel zenith ordering satisfies B > G > R for clear sky
  (catches dataset-channel-misalignment regressions).
- Zenith luminance at the noon reference (T = 2.5, albedo = 0.10,
  sun_alt = 60°) lies in the published 1–15 kcd/m² daylight range
  after the radiometric → photometric conversion.
- `cook` returns the zero sentinel below the horizon so the shader
  stays branch-free.

Primary implementation areas:

- `crates/astronomy/src/atmosphere.rs`
- `crates/astronomy/src/atmosphere/hosek_wilkie.rs`
- `crates/renderer/src/camera.rs`
- `crates/renderer/src/shaders/skyglow.wgsl`
- `apps/{cli,viewer,web}` host wiring + web UI controls.

### Observer-side light pollution — Bortle / SQM (`V-39` core)

Added an observer-side artificial-skyglow scaling of the dark-sky
background so the Bortle 8 Tokyo or downtown LA sky no longer renders as
a clean rural dark sky. The previous pipeline assumed a single ~21.6 mag/
arcsec² zenith for every observer regardless of location; V-39 lets a
host pick a Bortle 1-9 class or a hand-entered SQM mag/arcsec² value and
adds a sodium / LED warm-orange Garstang-scaled term into the diffuse-sky
composition before atmospheric extinction.

Shipped (core slice):

- `astronomy::skyglow::LightPollution { Bortle(u8), Sqm(f32),
  Atlas2016 { latitude_deg, longitude_deg } }` enum, plus
  `LightPollution::bortle_to_sqm_mag_per_arcsec2` (Bortle 2001 /
  Cinzano-Falchi-Elvidge 2001 typical zenith table; Class 5 anchored at
  V = 20.0 to satisfy the spec's calibration test), `artificial_zenith_s10`
  (excess over the natural floor), and `artificial_rgb_tint` (sodium / LED
  warm-orange linear-RGB tint normalised to luminance 1.0).
- `astronomy::skyglow::garstang_zenith_distance_kernel` ports the
  Garstang 1986 PASP 98, 364 single-scattering zenith-distance kernel,
  collapsed to the pure-observer-side scaling; clamped at 85° so the
  horizon glow stays finite. `artificial_skyglow_s10(pollution, z)` is the
  per-pixel evaluator.
- Renderer: `Camera::light_pollution` field, plus two new vec4 uniform
  fields `light_pollution_state` and `light_pollution_tint` in
  `CameraUniform`. The WGSL skyglow shader ports the Garstang kernel and
  adds the artificial term *before* extinction in the night-sky
  composition; Bortle 1 / dark-sky emits zero excess and the existing
  dark-sky composition stays bit-identical.
- Session schema bumped to **v5**: `SessionLightPollution { kind, bortle?,
  sqmMagPerArcsec2?, atlasLatitudeDeg?, atlasLongitudeDeg? }`. The `kind`
  tag is one of `bortle`, `sqm`, `atlas-2016`; range-checks reject
  out-of-band Bortle classes, SQM values, and lat/lng pairs.
- Host wiring: CLI / viewer add `--bortle`, `--sqm`,
  `--light-pollution-atlas LAT LNG`, and `--no-light-pollution` flags
  through a shared `LightPollutionOverrides` / `light_pollution_from_args`
  helper. Web exposes the matching WASM setter `set_light_pollution(
  enabled, kind, bortle_class, sqm, atlas_lat, atlas_lng)`. The TS
  declaration adds the new entry; the React settings card is deferred to
  a follow-up PR but the default flow already round-trips Bortle 1
  through the WASM ABI.
- Gallery presets: `tokyo-bortle-8` (Bortle 8 + hazy-urban atmosphere over
  Tokyo evening) and `dark-sky-bortle-1` (Bortle 1 pinned over the existing
  dark-sky scene; expected to render byte-identically to `dark-sky.png`
  and currently does).

Tests pinned at the V-39 calibration / regression contracts:

- `bortle_5_zenith_matches_20_within_tolerance` is the validation gate:
  zenith V mag/arcsec² for Bortle 5 must land within ±0.2 of 20.0.
- `bortle_1_keeps_natural_floor` and the byte-identical `dark-sky-bortle-1`
  gallery PNG together pin that Bortle 1 emits zero artificial S10 and
  preserves the pre-V-39 dark-sky composition.
- `sqm_input_round_trips` checks that a hand-entered SQM reading round-trips
  through the artificial-S10 conversion to within 0.05 mag.
- `bortle_class_is_monotone_in_brightness` and `bortle_class_clamps_to_valid_range`
  cover ordering and input clamping.
- `garstang_kernel_is_one_at_zenith_and_rises` pins the horizon-brighter
  behaviour while keeping the kernel finite under the 85° clamp.
- `atlas2016_falls_back_to_rural_default` keeps the deferred follow-up's
  sentinel renderable.

Follow-up `V-39-Atlas` (shipped) — Falchi 2016 World Atlas loader:

- `astronomy::FalchiAtlas` (`light_pollution_atlas.rs`) parses a compact
  little-endian `FALATL01` lat/lng grid of total zenith V mag/arcsec² and
  bilinearly samples it by observer location, skipping `NaN` no-data cells
  (ocean / out-of-swath). The module is IO-free and unit-tested against a
  synthetic in-memory grid (corners, centre blend, bounds, NaN handling,
  longitude wrap, header/truncation errors).
- `scripts/fetch-falchi-atlas.sh` downloads the ~1 GB upstream GeoTIFF (DOI
  10.5880/GFZ.1.4.2016.001) and `scripts/build-falchi-atlas.py` resamples it
  to the `FALATL01` grid via `μ = 21.6 − 2.5·log10(1 + ratio)` (the renderer's
  natural floor), so atlas- and Bortle/SQM-derived brightness share one scale.
- `stars_host_common::{load_falchi_atlas, resolve_light_pollution}` read the
  grid named by `STARS_FALCHI_ATLAS` and map `LightPollution::Atlas2016`
  to a sampled `LightPollution::Sqm` at render time (CLI via the headless
  `render` path, viewer at scene→app handoff). The session keeps the
  `Atlas2016` (lat, lng) for reproducibility; when no grid is configured the
  variant falls back to the Bortle-1 floor, so the renderer, shaders, and
  session schema are all unchanged and committed presets stay byte-stable.
- The upstream GeoTIFF is recorded in `data/manifest.toml`
  (`falchi-2016-world-atlas`, kind `runtime-service`: no committed bytes) and
  documented in `DATA_SOURCES.md`. Web keeps the existing `Atlas2016`
  fallback (sampling a ~1 GB raster in-browser is out of scope).

Primary implementation areas:

- `crates/astronomy/src/skyglow.rs` — `LightPollution` enum, Bortle ⇒ SQM
  lookup, Garstang single-scattering kernel, `artificial_skyglow_s10`.
- `crates/renderer/src/camera.rs` — `Camera::light_pollution`,
  `CameraUniform::light_pollution_state`, `CameraUniform::light_pollution_tint`.
- `crates/renderer/src/shaders/skyglow.wgsl` — WGSL artificial-skyglow
  term added before extinction; `garstang_kernel` per-pixel evaluator.
- `crates/common/src/{lib,session,presets}.rs` — `LightPollutionOverrides`,
  `light_pollution_from_args`, `SessionLightPollution`, schema-v5 bump.
- `apps/{cli,viewer}/src/main.rs` — `--bortle` / `--sqm` /
  `--light-pollution-atlas` / `--no-light-pollution` flags.
- `apps/web/src/lib.rs` + `apps/web/frontend/src/stars-web.d.ts` —
  `set_light_pollution` WASM setter.

### Atmospheric scintillation (`V-24`)

Added a Young 1967 / Dravins 1997-98 weak-turbulence intensity-variance
model for an unaided 7 mm pupil and wired it into the renderer as a
per-star, time-varying flux modulation. The previous pipeline rendered
every star as a perfectly steady point source; now bright low-altitude
stars visibly twinkle, faint stars near the zenith stay almost steady,
and high-altitude observatories see the expected damping.

Shipped:

- `crates/astronomy/src/scintillation.rs`:
  `intensity_variance(altitude_rad, h_obs_m, pupil_mm, c_n2_scale) ->
  (sigma_sq, corner_hz)` with the Young 1967 10.66 · sec(z)³ · D⁻⁷ˣ³
  closed form plus an exponential observer-altitude damping term using a
  Hufnagel-Valley effective Cn² scale height of 4 km (the surface-layer +
  lower-troposphere turbulence that drives naked-eye twinkle is
  concentrated well below the 8 km pressure scale height used by V-37 /
  V-38). The calibration constant is pinned so that the default
  `c_n2_scale = 1.0` reproduces the Dravins 1997 amateur-site median
  σ ≈ 4 % at the zenith. `temporal_corner_hz(altitude_rad)` returns the
  low-pass corner of the temporal spectrum, scaling as `1 / √sec z`
  (Fresnel scale).
- `crates/renderer/src/camera.rs`: new `Scintillation { enabled,
  c_n2_scale, seed }` field on `Camera` (default-on, default-seeded), a
  `scintillation_params: [f32; 4]` vec on `CameraUniform`, and per-frame
  derivation of `(σ²_zenith, f_corner_zenith, seed_bits, t_seconds)`
  from the canonical (β, α, DU, h) atmosphere state and the observer's
  `jd_ut1`. The external galactic viewpoint and `Atmosphere::OFF`
  automatically zero the variance so off-Earth scenes stay deterministic.
- `crates/renderer/src/shaders/star.wgsl`: per-instance PCG-hashed,
  time-bin-interpolated noise field samples at three slightly offset
  times to produce the Dravins 1998 colour scintillation. The
  modulation multiplies the post-extinction RGB flux by `(1 + σ · n)`,
  clamped non-negative so the divergent very-low-altitude regime cannot
  invert the multiplier.
- Time source: `t = fract(jd_ut1) × 86400` keeps the f32 phase in a
  precision-safe window and makes two renders of the same session at
  the same simulated UT1 bit-identical.
- `crates/common`: `ScintillationConfig` + `scintillation_from_args`
  helper; `SessionScintillation` block in the JSON session schema,
  which bumps to v4. CLI / viewer flags: `--no-scintillation`,
  `--scintillation-scale`, `--scintillation-seed`. The web frontend
  mirrors the same state in `observer.ts`, `session.ts`, `storage.ts`,
  with `StarView.set_scintillation` exposed from WASM.
- All `docs/presets/sessions/*.json` regenerated under v4 (the
  scintillation block defaults to enabled with the calibrated scale);
  `data/manifest.toml` re-hashed by `make manifest-check`.

Tests pinned:

- Default `c_n2_scale = 1.0` returns σ_zenith within 5×10⁻⁴ of the
  Dravins amateur-site target.
- σ²(airmass = 5) > 10 × σ²(airmass = 1) and σ²(4 km observer) < σ²(sea
  level) by > 5 × (the two spec-required monotonicities).
- Larger telescope pupils crush σ² via the D⁻⁷ˣ³ aperture-averaging
  exponent.
- Corner frequency falls as `1/√sec z` exactly (relative tolerance
  10⁻⁶).
- `Scintillation::OFF`, `c_n2_scale = 0`, and NaN inputs all return
  zero variance safely.
- `SessionScintillation` round-trips through v4 JSON and back.

Primary implementation areas:

- `crates/astronomy/src/scintillation.rs`
- `crates/renderer/src/camera.rs`
- `crates/renderer/src/shaders/star.wgsl`
- `crates/common/src/{lib,session,presets}.rs`
- `apps/{cli,viewer,web}` host wiring + web frontend types.

## Observation planning and positional trust (legacy `Phase 2`)

The Library-track positional-precision items (`L-01`–`L-05`, `L-07`, `L-08`)
and the Visual-track refraction / Sun / Moon / planet / atmosphere items
(`V-29`–`V-36`) are implemented. Together they provide the default
precision and physical atmosphere layer expected of the current viewer.

### Time systems

Implemented explicit time scales:

- UTC for civil input;
- UT1 for Earth rotation;
- TAI and TT through the built-in leap-second table;
- approximate TDB for ephemeris use;
- optional DUT1 handling.

Primary implementation areas:

- `crates/astronomy/src/time.rs`
- `crates/astronomy/src/observer.rs`
- host time parsing in `crates/common`

### Stellar apparent-place corrections

Implemented renderer-wired corrections:

- proper motion from catalog data;
- IAU 2006 precession;
- compact IAU-2000-style nutation;
- equation-of-equinoxes sidereal-time wiring;
- first-order annual aberration;
- pressure / temperature scaled atmospheric refraction.

Primary implementation areas:

- `crates/astronomy/src/corrections.rs`
- `crates/renderer/src/camera.rs`
- `crates/renderer/src/vertex.rs`
- star shader path in `crates/renderer`

### Solar-system bodies

Implemented apparent / topocentric rendering inputs for:

- Sun;
- Moon;
- Mercury;
- Venus;
- Mars;
- Jupiter;
- Saturn;
- Uranus;
- Neptune.

The renderer receives apparent directions, angular sizes, phase information, and
body state suitable for visual rendering.

Primary implementation areas:

- `crates/astronomy/src/ephemeris.rs`
- `crates/renderer/src/camera.rs`
- `crates/renderer/src/skyglow.rs`
- host controls in `apps/cli`, `apps/viewer`, and `apps/web`

### Moon phase and Earth-shadow aid

Implemented renderer-driven Moon phase and lunar-eclipse umbral darkening aid.
This is intended as visual support, not a final eclipse-prediction product.

Primary implementation areas:

- `crates/astronomy/src/ephemeris.rs`
- `crates/renderer/src/skyglow.rs`
- `crates/renderer/src/shaders/skyglow.wgsl` (`lunar_phase_lambert`)

The lunar disk shader reconstructs the surface normal on the *near* (visible)
hemisphere of the Moon. Earlier the radial component used `+moon_dir`, which
models the far hemisphere and renders the complementary phase (a 76 % waxing
gibbous appears as a ~24 % waning crescent). The sign is now `-moon_dir`,
matching the geometric convention that `moon_dir` points from the observer to
the Moon.

### Titan (`V-52c` — Meeus-grade; full TASS1.7 follow-up `V-52c-TASS17`)

Third slice of `V-52` (planetary rings and moons): Titan now renders as
a point source ≈0.5′–3′ from Saturn (Saturn's brightest moon, V ≈ 8.4
at opposition), routed through the same magnitude-to-flux pipeline as
the planets and the V-52b Galilean moons, and labelled behind the
existing planet-labels overlay layer. Titan's orbital position comes
from Meeus 1998 *Astronomical Algorithms* ch. 45 — a verified
machine-readable simplification of the TASS theory of Vienne & Duriez
1995 (A&A 297, 588) that the `astro` crate already implements for all
eight major Saturnian moons. This ships at the same accuracy posture
the V-52b Galilean moons did (good for naked-eye / small-eyepiece
identification within a few arcseconds near J2000, drifting to
≈10–60″ over the ROADMAP ±100-yr budget). The full ~5″ / ±100-yr
TASS1.7 precision upgrade is tracked as the dedicated follow-on rung
`V-52c-TASS17`.

Primary implementation areas:

- `crates/astronomy/src/moons.rs`: extended with `TitanApparent`,
  `apparent_titan(jd)`, and `apparent_titan_topocentric(observer)`,
  mirroring the V-52b Galilean API one-for-one. A new private
  `titan_from_saturn` helper is the Saturn-side analogue of the
  existing `galilean_moons_from_jupiter` (same sky-plane basis and
  east/north → equatorial-vector arithmetic; only the parent-planet
  radius and the per-moon Meeus driver differ).
- `crates/renderer/src/camera.rs`: extends `PlanetUniforms` and
  `CameraUniform` with a `titan_eq_radius` / `titan_rgb_magnitude` /
  `titan_params` block (single-slot version of the V-52b
  `galilean_*` block), populated in `Camera::planet_uniforms` and
  routed through the existing `apparent_disk_direction_j2000`
  refraction pipeline.
- `crates/renderer/src/shaders/skyglow.wgsl`: new
  `titan_disk_radiance` evaluator added to the per-pixel sum, sibling
  to the V-52b `galilean_disk_radiance`. The scalar form (rather than
  iterating a 1-element array) keeps the inner loop free of
  array-bound math for the only Saturnian moon currently rendered.
- `crates/renderer/src/shaders/star.wgsl`: padding fields to keep the
  WGSL view of `CameraUniform` aligned with the host struct (same
  pattern as V-52a / V-52b).
- `crates/renderer/src/text.rs`: a single Titan label candidate
  registered inside the existing planet-labels overlay branch with
  the same priority offset and overlay-toggle gate as the Galilean
  group.

Shipped capabilities:

- Topocentric `(RA, Dec, distance_au, angular_radius_rad, magnitude)`
  from `apparent_titan_topocentric(observer)`.
- Apparent magnitude from Karkoschka 1998 (*Icarus* 133, 134) mean-
  opposition `V(1, 0) = −1.28` plus the standard `5·log10(r · Δ)`
  distance term, evaluated against Saturn's heliocentric / geocentric
  distances (Titan orbits ≤0.008 AU from Saturn, well inside the
  V-52c accuracy budget).
- Amber-haze surface colour tuned for naked-eye eyepiece use,
  softened toward the Sun-illumination chroma the same way the V-52b
  Galilean tints are softened against Jupiter.
- Sub-arcsecond apparent disk (≈0.4–0.9″ across one orbital period at
  J2000-era geometry); rendered as a point source for the same
  reason the Galilean moons are.
- Label shares the existing planet-labels overlay toggle and inherits
  the amber tint.
- Same `planets_enabled` host gate as Saturn itself, so CLI
  `--no-planets`, the viewer toggle, and the web `set_planets_enabled`
  WASM hook all turn Titan off in one motion. No new session schema
  field, no new UI control, no new WASM setter introduced in this
  slice.

Validation (Meeus-grade):

- `titan_stays_within_max_elongation_from_saturn` confirms Titan's
  apparent separation from Saturn stays within its tabulated maximum
  elongation (≲3.4′ at closest opposition) at three points across one
  Titonian orbital period.
- `titan_swings_across_one_full_orbital_period` confirms the sky-plane
  offset reverses (> 200″) across half a period of 15.95 d.
- `titan_has_plausible_magnitude_near_opposition` cross-checks the
  `V(1, 0) + 5·log10(r · Δ)` formula against the published V ≈ 8.3
  near the 2003-12 Saturn opposition within ±0.4 mag.
- `titan_angular_radius_is_sub_arcsecond_at_opposition` pins Titan as
  a sub-pixel point source at every supported FoV across a full
  orbital period.
- `titan_topocentric_matches_geocentric_within_parallax` pins the
  topocentric path agrees with the geocentric one within ≈5″ (the
  Earth-radius parallax bound at Saturn's mean distance).
- `titan_unit_direction_is_normalised` pins
  `TitanApparent::direction_equatorial` as a unit vector.
- `titan_separation_from_saturn_is_within_a_few_arcminutes_at_j2000`
  pins the headline configuration (Titan ≈3′ from Saturn) at J2000.
- `renderer::camera::tests::titan_uniform_matches_apparent_titan_at_j2000`
  / `titan_uniform_disabled_when_planets_off` pin the Pod / Zeroable
  uniform plumbing end-to-end.

Deliberately out of scope for this slice:

- The ROADMAP `~5″ / ±100-yr` accuracy gate — tracked by
  `V-52c-TASS17`, which will swap in the full Vienne & Duriez 1995
  TASS1.7 coefficient tables without changing the host-facing API.
- A `saturn-titan-eyepiece` validation-gallery scene preset —
  deferred to a small follow-up PR so the preset JSON-export pipeline
  and its round-trip tests bump together, mirroring the
  `saturn-eyepiece` and `jupiter-eyepiece` deferrals from V-52a /
  V-52b.
- The remaining seven Meeus-supported Saturnian moons (Mimas /
  Enceladus / Tethys / Dione / Rhea / Hyperion / Iapetus). They are
  fainter than Titan by 1–4 magnitudes and fall outside the
  renderer's default limiting magnitude in most scene presets. The
  uniform-block design leaves room for them to slot in next to the
  Galilean block whenever the renderer wants to render them.

References (also pinned in ROADMAP `V-52c`):

- Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 45
  ("Positions of the Satellites of Saturn").
- Vienne, A., Duriez, L. 1995, A&A 297, 588 (TASS1.7 — ROADMAP target
  for the `V-52c-TASS17` precision upgrade).
- Karkoschka, E. 1998, *Icarus* 133, 134 (Titan visual photometry,
  source of the `V(1, 0) = −1.28` reduced magnitude).
- Archinal, B. A. et al. 2018, CMDA 130, 22 (Titan physical radius
  2575 km from IAU WGCCRE 2015).

Hosts wired: CLI / viewer / web (all driven by the shared
`planets_enabled` flag).

---

### Galilean moons — Lainey 2006 L1.2 precision upgrade (`V-52b-E5`, pivot from Lieske 1998 E5)

Replaced the Meeus 1998 ch. 44 truncation that V-52b shipped with the
full Lainey, Duriez & Vienne 2006 L1.2 semi-analytic theory of the
Galilean satellites (A&A 456, 783). Apparent Jovicentric positions of
Io / Europa / Ganymede / Callisto now stay within ≈20″ of JPL Horizons
across the ROADMAP ±100-yr budget — a >10× tightening of the previous
Meeus-grade 200″ bound, with the worst-case Callisto out-of-plane
drift (≈180″ at the ±100-yr edge) eliminated outright.

**Pivot rationale.** This rung originally targeted the Lieske 1998 E5
trigonometric series. The published E5 coefficient tables are no longer
reachable from a reproducible sandbox (A&A `ds7367` PDF returns 404,
IMCCE FTP exposes only the Lainey L1.x family, the cococubed.com
Lieske `galsat` Fortran mirror is dead). The IMCCE L1.2 distribution
is the modern successor at equivalent accuracy class (≤5″/100 yr
against the underlying numerical integration), with a reachable
Fortran source and machine-readable coefficient file. We pivoted
targets while keeping the substitution-point API unchanged so the
renderer picks up the upgrade transparently.

**What L1.2 evaluates.** For each satellite the IMCCE `BisL1.2.dat`
table carries the trigonometric series of four orbital elements:
`a` (semi-major axis), `L` (mean longitude with a linear secular
term), `z = e·exp(iϖ)` and `ζ = sin(i/2)·exp(iΩ)`. Up to ≈160
terms per moon plus a degree-8 Chebyshev correction over the L1.2
validity window [J1140, J2760]. Elements are converted to Cartesian
via the IMCCE `ELEM2PV` Kepler-iteration kernel and rotated into the
J2000 mean equator/equinox frame using the embedded `(Ψ, I) = (ome,
ainc)` pole orientation.

Primary implementation areas:

- `crates/astronomy/src/moons/lainey_l1.rs` (new): Fortran-faithful
  Rust port of the IMCCE `DL1_2` evaluator. Parses the embedded
  `BisL1.2.dat` once on first use via a whitespace tokeniser that
  applies the Fortran D-exponent fixup, materialises the table into
  a `static OnceLock<L1Tables>`, and exposes
  `jovicentric_state_j2000(moon, jd) -> JovicentricState` returning
  position + velocity in km / km/s in the J2000 mean equator and
  mean equinox frame. The Kepler iteration, the
  `Rz(ome) · Rx(ainc)` rotation, and the Chebyshev correction are
  one-for-one with the IMCCE Fortran.
- `crates/astronomy/data/BisL1.2.dat` (new): the IMCCE coefficient
  table, 84 384 bytes, embedded via `include_str!` and pinned in
  `data/manifest.toml` as `lainey-2006-l12-galilean-coeffs`.
- `crates/astronomy/src/moons.rs`: the `lieske_e5` substitution point
  is renamed to `lainey_l1`. The caller now adds the moon's 3D L1.2
  J2000 position directly to Jupiter's km position instead of
  projecting onto a sky-plane east/north basis — simpler and lossless,
  retaining the moon's line-of-sight depth.
- `data/manifest.toml`: new embedded-artifact row for the L1.2
  coefficient table (with provenance, license, and the retrieval
  command).

Shipped capabilities:

- `apparent_galilean_moons{,_topocentric}` now route through the full
  L1.2 series. Public API is unchanged.
- The renderer's V-52b sprite path picks up the upgrade transparently
  across CLI / viewer / web with no host code changes.
- `make manifest-check` verifies both the embedded L1.2 coefficient
  table and the pinned Horizons fixture.

Validation (against `data/horizons_galilean_moons.csv`):

| Epoch | Io    | Europa | Ganymede | Callisto |
|-------|-------|--------|----------|----------|
| 1900  | 14.3″ |  0.9″  |   8.9″   |  15.8″   |
| 2000  |  4.3″ |  6.6″  |   7.1″   |   4.1″   |
| 2100  |  5.5″ |  1.9″  |   0.9″   |   2.4″   |

Gate constant `moons::tests::GALILEAN_MAX_OFFSET_ERR_ARCSEC = 20.0″`,
enforced by `moons::tests::galilean_matches_horizons_within_l1_budget`
at every fixture epoch and moon. The remaining ≈10″ at the 1900
edge is dominated by Earth-Jupiter vector reduction differences
(Horizons uses DE441 / IAU 2006 precession; L1.2 was fitted against
DE406); tightening below 5″ requires aligning the reduction and is
a documented follow-up.

Deliberately out of scope for this slice:

- Porting the V-52d shadow projection onto L1.2. The shadow producer
  in `jupiter_shadows.rs` still uses its own Meeus ch. 44 reproduction;
  consistent host-parity with V-52b is tracked as the follow-up rung
  `V-52d-L1.2`. The cross-path consistency test
  `earth_xy_matches_apparent_galilean_moons_at_j2000` is marked
  `#[ignore]` until that rung lands.
- Aligning the Earth-Jupiter reduction frame so the worst-case 1900
  residual drops below 5″.
- Velocity-branch consumers (the L1.2 series already produces
  velocities; we expose them via `JovicentricState::velocity_km_s`
  for future use but no current renderer path consumes them).

References:

- Lainey, V., Duriez, L., Vienne, A. 2006, A&A 456, 783 —
  *Synthetic representation of the galilean satellites orbital
  motions from L1 ephemerides* (the L1.2 publication).
- IMCCE 2006, *L1.2 distribution*,
  `ftp://ftp.imcce.fr/pub/ephem/satel/galilean/L1/L1.2/` — source
  `L1.2.f`, coefficient files, validation `TestL1.2.res`.
- Lieske, J. H. 1998, A&AS 129, 205 — the original E5 target,
  retained for citation completeness.
- JPL Horizons On-Line Ephemeris System
  (https://ssd.jpl.nasa.gov/horizons/) — reference for the ≤20″
  validation gate.

Hosts wired: unchanged — already CLI / viewer / web through V-52b.

Legacy notes (pre-pivot scaffold landing) follow below for historical
context; the substitution point module file was renamed in this PR
from `moons/lieske_e5.rs` to `moons/lainey_l1.rs`.

---

### Galilean moons — Lieske 1998 E5 precision upgrade scaffold (legacy `V-52b-E5` scaffold)

`V-52b-E5` is the precision-upgrade follow-on to `V-52b`: replace the
Meeus 1998 ch. 44 truncation currently producing the Jovicentric
sky-plane offsets with the full Lieske 1998 E5 trigonometric series so
the apparent positions of Io / Europa / Ganymede / Callisto stay
within ~5″ of JPL Horizons across the ROADMAP ±100-yr budget. The
full coefficient transcription requires its own dedicated validation
matrix; this slice lands the substitution-point module structure, the
pinned Horizons reference fixture, and the test gate the follow-up
PR will tighten.

Primary implementation areas:

- `crates/astronomy/src/moons/lieske_e5.rs` (new): substitution-point
  module with `JovicentricOffset`, `jovicentric_offset`, and
  `MoonSeriesShape`. The body of `jovicentric_offset` currently
  delegates to the Meeus truncation via the `astro` crate (same
  numerical result `V-52b` shipped) and carries a `TODO(V-52b-E5)`
  block sketching the future evaluator (per-moon ξ / V / ζ trig sums,
  the `dG` Jupiter–Saturn long-period inequality, and the `qqdot`
  rotation matrix into J2000 mean equator). `MoonSeriesShape` pins the
  Lieske 1998 per-moon term counts (Io: 10/41/7; Europa: 24/66/11;
  Ganymede: 31/75/13; Callisto: 49/89/18 — the same numbers Jay
  Lieske's reference `galsat` Fortran exposes), so the follow-up's
  coefficient parser has a free shape sanity-check.
- `crates/astronomy/src/moons.rs`: routes the Jovicentric offset
  computation through `lieske_e5::jovicentric_offset` and promotes
  `GalileanMoon::astro` to `pub(crate)` so the submodule can call into
  it. Public API (`apparent_galilean_moons{,_topocentric}`,
  `GalileanMoonApparent`) is unchanged — the renderer and hosts pick
  up the upgrade transparently when the precision PR lands.
- `data/horizons_galilean_moons.csv` (new, manifest id
  `horizons-galilean-moons-fixture`): geocentric ICRF apparent
  RA / Dec / range / range-rate for Jupiter + the four Galilean moons
  at three epochs spanning ±100 years (1900 / 2000 / 2100 UT).
- `scripts/fetch-horizons-galilean-moons.sh` (new): bash regenerator
  hitting the public JPL Horizons API.
- `data/manifest.toml`: new generated-artifact row for the fixture
  (with provenance, license, and the regeneration command).

Shipped capabilities:

- Substitution point exists and is reached by both the geocentric and
  topocentric Galilean-moon code paths. Replacing the body of
  `lieske_e5::jovicentric_offset` in a follow-on PR cascades through
  every host without further changes.
- The 5-row × 3-epoch Horizons fixture is now committed and pinned by
  `make manifest-check`.
- `moons::tests::meeus_grade_matches_horizons_within_meeus_budget`
  computes the *Jovicentric* sky-plane offset error between the
  current Meeus-grade model and the Horizons fixture for every
  (moon, epoch) pair (precession-invariant because moon and Jupiter
  share the rotation) and asserts it stays under the
  `MEEUS_GRADE_MAX_OFFSET_ERR_ARCSEC = 200.0`″ band. The `V-52b-E5`
  precision upgrade tightens that constant to ~5″ once the Lieske
  series is wired through.
- `lieske_e5::tests::series_shape_matches_lieske_galsat_counts` pins
  the per-moon ξ / V / ζ term counts so a transcribed coefficient
  table that diverges from Lieske 1998's published shape fails the
  build at parse time.

Validation observations (Meeus-grade, against the pinned Horizons
fixture):

- 1900-01-01 UT: max per-moon Jovicentric-offset error ≈37″
  (Ganymede), min ≈2″ (Io).
- 2000-01-01 UT: max ≈151″ (Callisto), driven by the out-of-plane
  Dec-component error the Meeus simplification drops.
- 2100-01-01 UT: max ≈180″ (Callisto), same failure mode.
- The in-plane RA component error stays under ≈45″ at every epoch;
  the Dec component is the weak axis the full Lieske E5 series
  closes.

Deliberately out of scope for this slice:

- The full Lieske 1998 E5 trigonometric series transcription
  (≈700 coefficients + ≈700 argument / rate pairs across the four
  moons, plus the `dG` long-period inequality and the rotation
  matrix). Tracked as the rest of `V-52b-E5`.
- Densifying the Horizons fixture (more epochs, topocentric sites
  for diurnal-parallax pins) — explicitly part of the precision PR.
- Velocity-branch outputs (`samjay` / `samjap` distinction in
  Lieske's reference Fortran). The renderer only needs positions,
  so the velocity branch is left out for now.

References (also pinned in ROADMAP `V-52b-E5`):

- Lieske, J. H. 1998, A&AS 129, 205 (E5 theory — the target).
- Lieske, J. H. 1977, A&A 56, 333 (E2 theory — introduces the
  ξ / V / ζ decomposition).
- Lieske, J. H. 1977, JPL Engineering Memorandum 314-112 (companion
  partials / barycenter-to-Jupiter correction routines).
- JPL Horizons On-Line Ephemeris System
  (https://ssd.jpl.nasa.gov/horizons/) — the reference the precision
  gate is anchored against.

Hosts wired: unchanged — already CLI / viewer / web through `V-52b`.

---

### Titan — TASS1.7 precision upgrade (`V-52c-TASS17`) — done

`V-52c-TASS17` replaced the Meeus 1998 ch. 45 truncation that `V-52c`
shipped with a faithful Rust port of the Vienne & Duriez 1995 TASS1.7
Titan theory, bringing Titan's apparent position to within ~5″ of JPL
Horizons across the ROADMAP ±100-yr budget (the TASS1.7 model itself
matches the IMCCE Fortran reference to <1e-10 AU; the residual is
dominated by Saturn's own VSOP87 ephemeris).

Primary implementation areas:

- `crates/astronomy/data/redtass7.dat` (new, vendored): the TASS1.7
  series block extracted from the IMCCE `tass17.f` distribution
  (satellites Mimas..Titan + Iapetus; Hyperion excluded because
  `CALCLON` fixes its proper longitude, leaving Titan unaffected).
  Regenerable byte-for-byte via `scripts/build-tass17.sh` (downloads
  `tass17.f` from IMCCE and `awk`-slices the series block); pinned in
  `data/manifest.toml` as `vienne-duriez-1995-tass17-titan-coeffs`.
- `crates/astronomy/src/moons/tass17.rs` (rewritten): a Rust port of
  the IMCCE subroutines `LECSER` (parser), `CALCLON` (proper
  longitudes), `CALCELEM` (Titan's six TASS elements), and `EDERED`
  (elements → Saturn-centred cartesian, mean ecliptic & equinox
  J2000). New public API `kronocentric_state_j2000(jd) ->
  KronocentricState` returns Titan's 3D J2000-equatorial
  position + velocity, mirroring the Galilean
  `lainey_l1::jovicentric_state_j2000`.
- `crates/astronomy/src/moons.rs`: `titan_from_saturn` now adds the
  3D state vector directly to Saturn's apparent position (replacing
  the old sky-plane east/north projection), with one parent-planet
  light-time retardation step (`jd − Δ/c`). Public `TitanApparent`
  surface unchanged, so the renderer and all hosts pick up the
  upgrade transparently. Dead `cross`/`normalise` helpers and a
  duplicate Saturn-radius constant removed; `SPEED_OF_LIGHT_KM_S`
  added.
- `data/manifest.toml`: new provenance row for `redtass7.dat`; also
  fixed a stale `lieske_e5.rs → lainey_l1.rs` reference in the
  Galilean fixture row.

Validation:

- `tass17::tests::matches_imcce_examp7_reference` pins Titan's
  position at three epochs against the IMCCE `EXAMP7.res` golden
  output to <1e-10 AU (bit-for-bit with the Fortran).
- `moons::tests::titan_matches_horizons_within_tass17_budget`
  tightened from the old 100″ band to
  `TASS17_MAX_OFFSET_ERR_ARCSEC = 5.0`″; achieved 0.1″ (J2000),
  3.8″ (1900), 3.1″ (2100).
- `tass17::tests::series_term_counts_match_redtass7` pins Titan's
  `[7, 36, 35, 22]` (p, λ, z, ζ) term counts so a corrupted series
  table fails at parse time; speed / distance sanity bands
  (≈5.57 km/s, ≈1.22 Gm) round out the suite.

Deliberately out of scope:

- Porting the `V-52d` Galilean-shadow projection onto the upgraded
  moon ephemerides remains the follow-up rung `V-52d-L1.2`; its
  cross-path consistency test stays `#[ignore]`d.
- Light-time retardation was added to the Titan path only (it pulls
  the 1900 epoch from 7.6″ → 3.8″); applying the same step to the
  Galilean L1.2 path is a possible future tightening, not required
  for this rung.

References (also pinned in ROADMAP `V-52c-TASS17`):

- Vienne, A. & Duriez, L. 1995, A&A 297, 588 (TASS1.7 — the target).
- Vienne, A. & Duriez, L. 1991, A&A 246, 619 (TASS predecessor;
  satellite-index conventions Titan inherits).
- Meeus, J. 1998, *Astronomical Algorithms*, ch. 45 (the low-precision
  truncation `V-52c` shipped, now superseded by the TASS1.7 port).
- IMCCE `tass17.f` / `EXAMP7.res` distribution — the Fortran reference
  the Rust port reproduces and the golden test is pinned against.
- JPL Horizons On-Line Ephemeris System
  (https://ssd.jpl.nasa.gov/horizons/) — the reference the precision
  gate is anchored against.

Hosts wired: unchanged — already CLI / viewer / web through `V-52c`.

---

### Galilean shadow transits on Jupiter (`V-52d`)

Fourth slice of `V-52` (planetary rings and moons): each Galilean
moon's silhouette now casts a dark spot on the Jovian disk during
shadow transits, and a moon disappears whenever it sits behind
Jupiter from the observer's line of sight. The shadow geometry reuses
the V-51b analytic-mask occluder array end-to-end — no new shader
target was needed; the same Planet-on-Planet path the V-51d / V-51f
slices ship already routes the dark front-disk into Jupiter's pixel
source term.

Primary implementation areas:

- `crates/astronomy/src/jupiter_shadows.rs` (new): re-implements the
  Meeus 1998 ch. 44 truncated series to expose **3D Jovicentric
  rectangular coordinates** of each Galilean moon — once from the
  Earth's line of sight (`earth_xyz_r_j`) and once from the Sun's
  (`sun_xyz_r_j`) — in units of Jupiter's equatorial radius. The
  Earth view drives moon-behind-Jupiter / moon-in-front-of-Jupiter
  classification; the Sun view drives shadow projection onto the
  Jovian disk. `galilean_shadow_disks_at` returns the ready-to-pack
  analytic disks for the V-51b occluder array, with each shadow's
  apparent radius = `moon.radius_km / earth_jupiter_distance_km`
  (the moon's silhouette spans the same physical extent on Jupiter
  as the moon itself, so its apparent size from Earth is just the
  moon's physical radius at the Jupiter range).
- `crates/astronomy/src/planning.rs`: `active_occluders` emits one
  `OccluderTarget::Planet(3)` (Jupiter) entry per active shadow,
  using `OccultationKind::AnnularOrTransit` (the front disk is
  always strictly smaller than the back disk). Off-event the
  producer pushes zero entries, so frames far from any Galilean
  transit stay bit-identical to the pre-V-52d render.
- `crates/renderer/src/camera.rs`: when a Galilean moon currently
  sits behind Jupiter from the observer, the renderer packs a
  **negative** angular-radius sentinel into the V-52b
  `galilean_eq_radius[i].w` slot. The shader treats negative radii
  as the "hidden" cull. Naked-eye-FoV frames whose moons are all
  outside Jupiter's silhouette stay bit-identical.
- `crates/renderer/src/shaders/skyglow.wgsl`:
  `galilean_disk_radiance` short-circuits on the negative-radius
  sentinel so a moon's point sprite disappears while it transits
  behind Jupiter. The V-51b `planet_disk_radiance` path that
  already subtracts front disks from Jupiter handles the shadow
  spot itself with no shader change.
- Scene preset: `jupiter-shadow-transit`
  (`docs/presets/sessions/jupiter-shadow-transit.json`) frames the
  2008-12-20 14:00 UT Io shadow transit from Roque de los
  Muchachos (Canary Islands, where Jupiter rides ~39° up at the
  pinned epoch) with a 0.05° eyepiece field, so Io's silhouette
  is the dominant pixel feature.

Shipped capabilities:

- Per-moon Jovicentric 3D state from both Earth and Sun
  perspectives, with closed-form predicates for "shadow on
  Jupiter", "moon in front of Jupiter", and "moon behind Jupiter"
  (`crates/astronomy/src/jupiter_shadows.rs::GalileanShadowState`).
- V-52d shadow transit drawn as one analytic-mask occluder per
  active moon, routed through the same V-51b uniform path as
  V-51d / V-51e / V-51f.
- Moon-behind-Jupiter sprite cull driven from the same producer.

Validation (Meeus-grade):

- `io_shadow_ingress_within_five_minutes_of_horizons_2008_12_20`
  pins the 2008-12-20 Io shadow-transit ingress within ±5 min of
  the geocentric PHEMU09 / JPL Horizons reference (13:14 UT). The
  V-52d roadmap test gate is "within 5 min of JPL Horizons".
- `earth_xy_matches_astro_apprnt_rect_coords_at_j2000` and
  `earth_xy_matches_apparent_galilean_moons_at_j2000` lock the
  Earth-view geometry against the V-52b renderer ephemeris path
  (no drift between shadow producer and moon sprite).
- `shadow_radius_matches_moon_radius_at_jupiter_distance` checks
  the shadow disk's angular radius matches `R_moon / Δ_Jupiter`
  exactly.
- `shadow_disk_direction_close_to_jupiter` confirms an active
  shadow's sky-plane direction stays inside one Jovian apparent
  radius of Jupiter's centre during the pinned mid-transit.
- `planning::active_occluders_emit_io_shadow_at_2008_12_20_transit`
  pins the V-52d producer's push of exactly one Planet(Jupiter)
  entry at the 2008-12-20 14:00 UT epoch with the right radius
  scale, kind code, and obscuration ratio.
- `planning::active_occluders_emit_no_galilean_shadow_off_event`
  pins the off-event producer contract on a quiet date (no
  Planet(Jupiter) entries with Galilean-sized front-disk radii).
- `camera::occluder_uniform_emits_io_shadow_at_2008_12_20_transit`
  pins the renderer uniform path: target code = 5 (Planet(3)),
  unit-length direction, plausible Io silhouette radius, and the
  `AnnularOrTransit` kind code.

Deliberately out of scope for this slice:

- Moon ↔ moon mutual occultation (a moon hiding another moon).
  The V-51b `OccluderTarget` enum reserves codes only for the
  Sun, the Moon, the seven planets, and the star cull; encoding
  the four Galilean moons individually would require either a
  new target enum or a per-moon analytic-mask extension in
  `shaders/skyglow.wgsl`. PHEMU-cadence moon ↔ moon events are
  rare (≲ once per year, mostly outside opposition) so the
  deferral keeps the V-52d shadow / behind-Jupiter scope clean.
  The 3D `earth_xyz_r_j` state needed to drive moon-on-moon
  classification is already produced by
  `galilean_shadow_states`, so the future slice only needs the
  occluder-target plumbing.
- The ROADMAP `~5″ / ±100-yr` accuracy gate — still owned by
  `V-52b-E5`, which will swap in the full Lieske 1998 series
  for both the moon and shadow positions without changing the
  host-facing API.

References (also pinned in ROADMAP `V-52d`):

- Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 44
  ("Positions of the Satellites of Jupiter"), eq. 44.1–44.10 +
  the shadow-projection note.
- Lieske, J. H. 1998, A&AS 129, 205 (E5 theory — ROADMAP target
  for the `V-52b-E5` precision upgrade).
- IMCCE PHEMU09 working group for the 2008–2009 mutual-event
  campaign tables used to pin the ingress test gate.

Hosts wired: CLI / viewer / web (all driven by the shared
`planets_enabled` flag — same gate as Jupiter itself).

---

### Galilean moons (`V-52b`)

Second slice of `V-52` (planetary rings and moons): Io, Europa,
Ganymede, and Callisto now render as point sources next to Jupiter,
routed through the same magnitude-to-flux pipeline as the planets and
labelled behind the existing planet-labels overlay layer. Their
orbital positions come from Meeus 1998 *Astronomical Algorithms* ch. 44
(a low-precision reduction of Lieske's E5 theory) — enough for
naked-eye / small-eyepiece identification across the next few decades
but not the full ROADMAP ±100-yr / ~5″ accuracy gate. The precision
upgrade is tracked as the dedicated follow-on rung `V-52b-E5`.

Primary implementation areas:

- `crates/astronomy/src/moons.rs` (new): `GalileanMoon` enum,
  `GalileanMoonApparent`, and the
  `apparent_galilean_moons{,_topocentric}` API mirroring the planet /
  Saturn-ring shape.
- `crates/astronomy/src/ephemeris.rs`: four equatorial / observer
  helpers promoted to `pub(crate)` so the new module can reuse them
  without duplicating SOFA-style math.
- `crates/renderer/src/camera.rs`: extends `PlanetUniforms` and
  `CameraUniform` with a `galilean_eq_radius[4]` /
  `galilean_rgb_magnitude[4]` / `galilean_params` block, populated in
  `Camera::planet_uniforms` and routed through the existing
  `apparent_disk_direction_j2000` refraction pipeline.
- `crates/renderer/src/shaders/skyglow.wgsl`: new
  `galilean_disk_radiance` evaluator added to the per-pixel sum.
- `crates/renderer/src/shaders/star.wgsl`: padding fields to keep the
  WGSL view of `CameraUniform` aligned with the host struct (same
  pattern as `V-52a`).
- `crates/renderer/src/text.rs`: `GALILEAN_LABELS` registered as a
  candidate batch inside the existing planet-labels overlay branch.

Shipped capabilities:

- Per-moon topocentric `(RA, Dec, distance_au, angular_radius_rad,
  magnitude)` from `apparent_galilean_moons_topocentric(observer)`,
  with the four moons returned in Lieske's canonical I-II-III-IV order.
- Apparent magnitudes from Meeus 1998 table 41.A reduced `V(1,0)`
  values plus the standard `5·log10(r·Δ)` distance term, evaluated
  against Jupiter's heliocentric / geocentric distances.
- Surface colour palette tuned for naked-eye eyepiece use (Io: sulfur
  yellow; Europa: water-ice white; Ganymede: tan-grey; Callisto: dark
  grey-tan); rendered as point sources because the largest Galilean
  disc (Ganymede at ~1.7″) is sub-pixel at every supported FoV.
- Labels share the existing planet-labels overlay toggle and inherit
  the same colour family.
- Same `planets_enabled` host gate as Jupiter itself, so CLI
  `--no-planets`, the viewer toggle, and the web `set_planets_enabled`
  WASM hook all turn the moons off in one motion. No new session
  schema field or UI control was added in this slice.

Validation (Meeus-grade):

- `moons_returned_in_canonical_order` pins the I-II-III-IV ordering of
  `GalileanMoon::ALL`.
- `moons_stay_within_max_elongation_from_jupiter` confirms each moon's
  apparent separation from Jupiter at J2000 stays within its tabulated
  maximum elongation (Io ≲ 2.4′, Callisto ≲ 10.7′ at closest opposition).
- `moons_have_distinct_positions` pins pairwise separations > 1″ at
  J2000.
- `moons_have_plausible_magnitudes_near_opposition` cross-checks the
  `V(1,0) + 5·log10(r·Δ)` formula against the standard near-opposition
  V values `[5.0, 5.3, 4.6, 5.7]` within ±0.4 mag.
- `moons_evolve_over_one_io_period` confirms Io's sky-plane offset
  reverses across half its 1.77-day orbital period.
- `topocentric_matches_geocentric_within_parallax` checks the
  topocentric path agrees with the geocentric one within ≈10″ (the
  Earth-radius parallax bound at Jupiter's mean distance).
- `moon_unit_direction_is_normalised` pins
  `GalileanMoonApparent::direction_equatorial` as a unit vector.

Deliberately out of scope for this slice:

- The ROADMAP `~5″ / ±100-yr` accuracy gate — tracked by
  `V-52b-E5`, which will swap in the full Lieske 1998 series without
  changing the host-facing API.
- `jupiter-eyepiece` validation-gallery scene preset — deferred to a
  small follow-up PR so the preset JSON-export pipeline and its
  round-trip tests bump together, mirroring the `saturn-eyepiece`
  deferral from `V-52a`.
- Shadow / occultation transits on the Jovian disk and mutual
  occultations between the moons — owned by `V-52d`, which will
  reuse the geometry produced here and the `V-51b` analytic-mask
  occluder array.

References (also pinned in ROADMAP `V-52b`):

- Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 44
  ("Positions of the Satellites of Jupiter").
- Lieske, J. H. 1977, A&A 56, 333 (E2 theory — origin of the
  coefficient family Meeus simplifies).
- Lieske, J. H. 1998, A&AS 129, 205 (E5 theory — ROADMAP target for
  the `V-52b-E5` precision upgrade).
- Archinal, B. A. et al. 2018, CMDA 130, 22 (Galilean physical radii
  and IAU WGCCRE rotation parameters).

Hosts wired: CLI / viewer / web (all driven by the shared
`planets_enabled` flag).

---

### Double / binary star resolution (`V-54`)

Tight visual doubles that HYG v4.2 merges into a single catalog row now
split into two component sprites at the Washington Double Star (WDS)
separation `ρ` and position angle `θ`, each carrying its own B−V colour
through the `V-23` blackbody pipeline. The split happens at catalog-load
time, so all three hosts (CLI / viewer / web) resolve the same pairs with
no host-specific wiring.

The shipped bootstrap covers the genuinely-merged showpiece pairs:
**Algieba** (γ Leo, one merged HYG row → golden A/B) and the **ε Lyrae
"Double Double"** (HYG ids 91633 / 91639, each itself an unresolved pair
→ four resolved sprites). Pairs HYG already ships as two rows are
excluded to avoid double-counting — notably **Mizar** (A = id 65173, B =
id 118887 at ~19″, Alcor = id 65272 at ~12′), Albireo (β1/β2 Cyg), and
Castor (α Gem A/B); their split / gold-blue appearance already flows
through the existing catalog + `V-23` pipeline.

Primary implementation areas:

- `crates/catalog/data/double_stars.csv` (new): WDS-derived table keyed
  by the merged HYG primary `id` with `(ρ, θ, m1, m2, B−V1, B−V2, epoch,
  WDS id)`. Regenerated by `scripts/extract-double-stars.py` and pinned
  in `data/manifest.toml` as `wds-double-stars-bootstrap`.
- `crates/catalog/src/doubles.rs` (new): parses the table via `OnceLock`
  and `resolve_doubles(Vec<Star>) -> Vec<Star>` replaces each matching
  merged primary with two components (primary at the catalog position,
  secondary offset by the exact great-circle step at `(ρ, θ)`). Matches
  by HYG `id` when present, else by position (15″ tolerance) so the
  identifier-less embedded binary path resolves the same pairs.
- `crates/catalog/src/catalog.rs`: both `load_from_csv` (native HYG CSV)
  and `load_from_binary` (embedded / web) call `resolve_doubles`.

Why it counts as complete: the merged showpiece pairs resolve into
correctly-placed, correctly-coloured component sprites across all hosts,
the behaviour is pinned by unit tests, and the manifest / docs are in
sync.

Acceptance threshold: the native hosts build the GPU instance buffer once
and only vary FOV per frame, so both components are always emitted and
the "merged below 1 px" appearance is an emergent property of the
renderer's *linear* HDR PSF accumulation (`V-16` / `V-17`) — two
sub-pixel components of flux `f1`, `f2` sum to one PSF of flux `f1 + f2`.
A pinned test confirms the combined component magnitude stays within
~0.1 mag of the original merged magnitude, so no brightening or aliasing
is introduced at any zoom.

Tests / validation (`crates/catalog/src/doubles.rs`): Algieba splits at
the WDS 4.6″ separation / 126° position angle (by id and by position),
the ε Lyrae pair yields four sprites, combined flux stays within 0.15 mag
of the merged HYG magnitude, Albireo's existing rows render a gold/blue
pair through `V-23`, the already-resolved Mizar B row is not re-split,
and ordinary stars pass through unchanged.

Hosts wired: CLI / viewer (HYG CSV path) / web (embedded binary path).

---

### Resolved open clusters (`V-53`)

Bright open clusters that V-42 drew as a single DSO marker (Pleiades M45,
Praesepe / Beehive M44, Double Cluster NGC 869 / NGC 884) now render as a
*resolved field of HYG stars* with the cluster label sitting over the
field. This matches what a naked-eye observer actually sees and removes
the misleading disk-shaped marker that previously sat on top of the
Pleiades' seven sisters.

Primary implementation areas:

- `crates/catalog/data/cluster_membership.csv` (new): committed
  membership table joining HYG / Hipparcos IDs to a parent open
  cluster's `M<N>` / `NGC<N>` identifier. Provenance pinned in
  `data/manifest.toml` as `open-cluster-membership-bootstrap`.
- `crates/catalog/src/clusters.rs` (new): parses the CSV exactly once
  via `OnceLock`, exposes
  `cluster_members(DeepSkyId) -> &'static [ClusterMember]` and
  `is_resolved_as_member_field(DeepSkyId) -> bool`.
- `crates/catalog/src/deepsky.rs`: extends `DeepSkyCatalog` with
  `resolve_as_member_field(DeepSkyId) -> bool` (default `false`);
  `MessierCatalog` and `NgcBrightCatalog` delegate to the cluster
  module.
- `crates/renderer/src/overlay.rs`: `deep_sky_markers` skips marker
  geometry for any DSO that the catalog tags as resolve-into-member-field.
  The label pass is intentionally untouched — the cluster label still
  draws over the resolved star field.
- `scripts/extract-cluster-membership.py` (new): regenerates the CSV
  byte-identically from the hand-curated bootstrap list; documents the
  Cantat-Gaudin 2020 follow-up path via `--from-cantat-gaudin` (stubbed).

Shipped capabilities:

- Pleiades (M45): 9 named-star members (Alcyone, Atlas, Electra, Maia,
  Merope, Taygeta, Pleione, Asterope, Celaeno). The disk-shaped marker
  no longer hides the seven sisters.
- Praesepe / Beehive (M44): 11 brightest core members at V ≤ 6.9.
- Double Cluster (NGC 869 + NGC 884): the HYG-resolvable bright
  members split by RA (RA < 2.345 → NGC 869, ≥ 2.345 → NGC 884).
- Density slider compatibility: the existing
  `OverlayConfig::deep_sky_magnitude_limit` slider continues to gate
  every other DSO; the four resolved clusters are unconditionally
  suppressed because their members are part of HYG and respond to the
  star-magnitude controls instead.

Validation:

- `pleiades_named_seven_positions_match_within_one_arcminute` resolves
  HYG positions for the seven named bright Pleiades stars (Alcyone,
  Atlas, Electra, Maia, Merope, Taygeta, Pleione) and asserts each is
  within 1' of its SIMBAD / Hipparcos reference position — the V-53
  validation gate from ROADMAP.
- `pleiades_named_seven_are_members`,
  `praesepe_is_resolved_as_member_field`,
  `double_cluster_is_resolved_as_member_field`,
  `unrelated_dso_is_not_resolved_as_member_field`,
  `resolved_cluster_ids_match_v53_scope` pin the membership table's
  scope.
- `deep_sky_markers_suppress_v53_resolved_clusters` asserts the renderer
  drops marker geometry for the four resolved clusters and keeps it for
  unrelated DSOs (M31, NGC 7000).
- `deep_sky_markers_at_show_all_limit_have_expected_segment_count`
  updated to reflect the two-Messier-diamond suppression.
- `stars-manifest` re-hashes `cluster_membership.csv` against the
  pinned `data/manifest.toml` row on every `make ci`.

Deliberate scope for this first slice:

- Hand-curated showpiece bootstrap (4 clusters / 34 member rows) rather
  than the full Cantat-Gaudin 2020 catalog. The extractor script is in
  place; the `--from-cantat-gaudin` switch will swap in the full
  Gaia DR2/DR3 membership table in a follow-up PR without changing the
  CSV's column shape.
- Hyades (Mel 25) is intentionally deferred: it has no current V-42 DSO
  marker to suppress (no Messier number, not in `openngc_bright`), and
  its label asset will be added together with the Cantat-Gaudin upgrade.
- No globular-cluster star-by-star resolution and no cluster colour-
  magnitude diagrams (ROADMAP non-goals).
- The Double Cluster members reflect HYG v4.2's V ≤ 9 truncation — the
  cluster's photometric core (V ~ 9–13) is below HYG depth and will only
  appear once a deeper background-star catalog is wired.

References:

- Cantat-Gaudin, T. et al. 2020, A&A 633, A99 (DOI
  10.1051/0004-6361/201936691, "Painting a portrait of the Galactic disc
  with its stellar clusters").
- Mermilliod, J.-C. & Paunzen, E. 2003, A&A 410, 511 (WEBDA database).

Hosts wired: CLI / viewer / web (the catalog crate is the single seam
all three consume; no host-side knob was added).

---

### Saturn ring system (`V-52a`)

First slice of `V-52` (planetary rings and moons): Saturn now renders
with its A / B / C ring bands and the Cassini Division, opened by the
sub-Earth Saturnicentric latitude `B`, and shadowed where the planet
body sits between the observer and the rear half of the ring plane.
The other three rungs of `V-52` (Galilean moons, Titan, Galilean
shadow / occultation transits) remain open and ship in subsequent PRs.

Primary implementation areas:

- `crates/astronomy/src/ephemeris.rs`: new `SaturnRingApparent` and
  `apparent_saturn_ring{,_topocentric}` API.
- `crates/renderer/src/camera.rs`: `CameraUniform::saturn_ring_pole_sinb` /
  `saturn_ring_state` + `PlanetUniforms` Saturn-ring block.
- `crates/renderer/src/shaders/skyglow.wgsl`: `saturn_ring_brightness`
  evaluator wired into `planet_disk_radiance` for the Saturn entry.
- `crates/renderer/src/shaders/star.wgsl`: padding fields to keep the
  WGSL uniform binding aligned with the host struct.

Shipped capabilities:

- Ring orientation: ring-plane inclination `i = 28.075216° − 0.012998° T
  + 0.000004° T²` and ascending-node longitude `Ω = 169.508470° +
  1.394681° T + 0.000412° T²` from Meeus 1998 ch. 45.
- Sub-Earth latitude `sin B = sin i · cos β · sin(λ − Ω) − cos i · sin β`
  using Saturn's apparent geocentric ecliptic position (λ, β) from
  `apparent_planet(Planet::Saturn)`.
- Sub-Sun latitude `sin B'` computed analogously from the heliocentric
  ecliptic longitude / latitude returned by `astro::planet::heliocent_coords`.
- Ring pole direction in the equatorial frame of date, rotated through
  the same J2000 path as the planet direction so the shader sees both
  in one consistent frame.
- Renderer ring shader: builds a sky-tangent basis at Saturn's centre,
  decomposes each ray into `(u, v)` along the projected ring major and
  minor axes, de-projects `v` by the `|sin B|` foreshortening to recover
  the true ring-plane radius in Saturn-radius units, looks up the band
  (C / B / Cassini Division / A) and weights by Dones et al. 1993
  brightness ratios `[0.20, 1.00, 0.15, 0.50]`.
- Body-on-ring shadow: ring pixels on the half of the ring whose `v`
  has the opposite sign to `sin B` (i.e., the far half) are occulted
  when their sky-plane offset falls inside Saturn's body silhouette.
- Lit / unlit face: when `sign(sin B) ≠ sign(sin B')` the side facing
  the observer is the unlit face; the ring drops to a 10 % factor
  (ringshine / forward-scattered Saturnshine).
- Ring-radius constants pinned to the Cassini orbital fits (Porco et
  al. 2005), C-inner 1.236, B-inner 1.526, B-outer 1.951, A-inner 2.025,
  A-outer 2.270 × Saturn equatorial radius (60 268 km, IAU WGCCRE 2015).
- Pixel-scale floor: the ring tracks the body's per-pixel-floored
  visual radius, so at naked-eye FoV where Saturn's true 9″ disk is
  sub-pixel, the ring scales up with the body instead of vanishing.

Tests / validation (`crates/astronomy/src/ephemeris.rs`):

- `saturn_ring_edge_on_in_1995`: ring-plane crossing 1995-08-10,
  |B| < 0.6°.
- `saturn_ring_max_open_in_2002`: southern-face maximum 2002-12-17,
  B = (−26.7 ± 0.3)°, `sign(B') = sign(B) < 0`.
- `saturn_ring_edge_on_in_2009`: ring-plane crossing 2009-09-04,
  |B| < 0.6°.
- `saturn_ring_max_open_north_in_2017`: northern-face maximum
  2017-05-28, B = (+26.7 ± 0.5)°, `sign(B') = sign(B) > 0`.
- `saturn_ring_pole_is_unit_length_and_near_iau_pole_at_j2000`:
  ring pole within 0.1° of the IAU WGCCRE 2015 pole
  `α₀ = 40.589°, δ₀ = 83.537°` at J2000.
- `saturn_ring_topocentric_matches_geocentric`: Earth-radius parallax
  cannot move the ring orientation at the V-52a accuracy budget.

Deliberately out of scope for this slice:

- Ring shadow on the planet body (the equatorial dark band on Saturn's
  southern hemisphere when north face is open). Visible at telescope
  scale but not at naked-eye scale; deferred until the V-52a renderer
  pipeline is in place.
- `saturn-eyepiece` validation-gallery scene preset — deferred to a
  small follow-up PR so the preset JSON-export pipeline and its
  round-trip tests bump together.

References (also pinned in ROADMAP `V-52a`):

- Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 45.
- Porco, C. C. et al. 2005, Science 307, 1226.
- Dones, L. et al. 1993, Icarus 105, 184.
- Archinal, B. A. et al. 2018, CMDA 130, 22.

Hosts wired: CLI / viewer / web.

---

### Spectral airglow decomposition (`V-28`)

Replaced the single dark-sky airglow floor (one S10 constant that fed all
RGB channels equally through a cool-white tint) with the three dominant
atmospheric emission systems: O I 557.7 nm green line, Na D 589 nm, and
the OH Meinel red/IR bands. This is what gives a real dark-site night
sky its characteristic faint green/red mottled tint and removes the
unphysical pure-grey night floor.

Scientific basis. Leinert et al. 1998 §7.5 tabulates the zenith airglow
for a "moderate-activity" night: green line ≈ 250 R, Na D ≈ 30 R, OH
Meinel ≈ 800 R integrated through the V band. The V-band-weighted S10(V)
split used here — 80 + 15 + 50 — sums to the same ≈ 145 S10(V) floor
that V-13 / V-21 were tuned against, so the total dark-sky luminance is
preserved at zenith. Each layer is brightened toward the horizon by a
Van Rhijn integral `1 / sqrt(1 − (R/(R+H))² sin² z)` evaluated with its
own layer altitude (90 km O I, 92 km Na D, 87 km OH); at the geometric
horizon every component reaches ≈ 5× its zenith value.

Per-channel chromaticity uses fixed linear-sRGB tint vectors normalised
to Rec. 709 luminance Y = 1, so multiplying the V-band S10 contribution
of a line by its tint vector preserves the V-band luminance budget while
giving each component its characteristic colour: green dominates G,
Na D yellow contributes warm R + G, and the OH red/IR tail in V drives R.
There is no B-channel contribution by construction.

Primary implementation areas:

- `crates/astronomy/src/skyglow.rs`: new `airglow_components(altitude_rad,
  activity_level) -> (green, sodium, oh)` and `airglow_rgb_s10(
  altitude_rad, activity_level) -> [R, G, B]` evaluators plus the
  `van_rhijn_factor(altitude_rad, layer_height_km)` helper and the
  `AIRGLOW_{GREEN,SODIUM,OH}_RGB` chromaticity constants;
  `diffuse_sky_mag_per_arcsec2` now folds in the V-band sum of the three
  components instead of the hard-coded `145 S10(V)` floor.
- `crates/renderer/src/shaders/skyglow.wgsl`: removed the
  `let airglow = 145.0` term from `diffuse_sky_mag_per_arcsec2` and added
  a parallel `airglow_radiance_rgb(altitude_rad, zeropoint,
  pixel_arcsec2)` evaluator that mirrors the Rust API. The fragment
  shader now sums `tint * isl_zl_flux + airglow_rgb_flux` before
  applying the per-channel Kasten-Young extinction, so the airglow tint
  remains visible exactly where the rest of the dark-sky model is
  visible (above the geometric horizon under any non-OFF atmosphere
  preset).

Validation (`crates/astronomy/src/skyglow.rs::tests`):

- `airglow_zenith_total_matches_leinert` — zenith total integrated
  airglow in V band is within 10 % of the Leinert §7 reference
  (145 S10(V) at moderate activity). Pinned per the ROADMAP V-28 spec.
- `airglow_chromaticity_differs_from_neutral_grey` — |R−G|/Y ≥ 0.10
  (documented threshold for the V-28 "removes the grey-floor"
  acceptance), B/Y < 0.02, and per-channel V-band luminance equals the
  V-band S10 sum (the chromaticity vectors are luminance-preserving by
  construction).
- `airglow_horizon_brighter_than_zenith` — every component is 4.5–6.5 ×
  zenith at the geometric horizon (Van Rhijn limb brightening).
- `van_rhijn_zenith_to_horizon` — the closed-form Van Rhijn factor
  reproduces 1.0 at zenith and ≈5–6 at the horizon for a 90 km layer.
- `airglow_activity_scaling_is_linear` — the `activity_level` knob is a
  uniform multiplier across the three components.

Deliberately out of scope for this slice. Activity-level scaling is a
Rust API knob; no host uniform / UI control is added yet. The shader
path hard-codes the Leinert moderate-activity reference. A follow-up
could plumb the activity scale through the V-37 atmosphere state if a
user-visible "solar-cycle" knob is wanted; the colour split would not
need to change.

References (also pinned in ROADMAP `V-28`):

- Leinert, Ch. et al. 1998, A&AS 127, 1, §7.4–7.6.
- Krassovsky, V. I., Shefov, N. N., Yarin, V. I. 1962, Planet. Space
  Sci. 9, 883 (OH Meinel bands).
- Roach, F. E. & Gordon, J. L. 1973, *The Light of the Night Sky*.

Hosts wired: CLI / viewer / web (all driven through the existing
skyglow shader pass).

---

### Mutual planetary occultation (`V-51f`)

Sixth slice of `V-51` and the last open producer in the unified
eclipse / occultation pass: the seven rendered planets now occult each
other when one planet's apparent disk passes in front of another's.
The V-51b general `MAX_OCCLUDERS = 16` analytic-mask array and the
`OccluderTarget::Planet(i)` shader path were already wired by V-51d
(Moon-on-Planet), so this slice is a producer-side change plus a
new planning-side helper. Mutual planetary occultations are rare in
practice (the next visible event is 2065-11-22 Venus occults Jupiter),
so historical-event positive-detection validation is deferred until
the `L-06` DE440 upgrade lands; producer-contract and same-planet /
off-event rejection are pinned today.

Primary implementation areas:

- `crates/astronomy/src/planning.rs` (`active_occluders` extended
  with a Planet-on-Planet sub-producer; new
  `find_mutual_planetary_occultation` planning helper +
  `MutualPlanetaryOccultationEvent` type).
- `crates/astronomy/src/lib.rs` (re-exports for the new helper and
  event type).
- `crates/renderer/src/shaders/skyglow.wgsl` (comment updates only —
  the analytic-mask path is unchanged because the V-51d
  `OCCLUDER_TARGET_PLANET_BASE + i` lookup already subtracts any
  Planet-targeted occluder from planet `i`'s disk).

Shipped capabilities:

- `active_occluders` now precomputes the seven apparent planet disks
  once per call, then iterates unordered planet pairs `(i, j)` with
  `i < j`. For each pair the closer planet (smaller `distance_au`) is
  assigned as the front disk and the farther as the back; the pair is
  classified via the V-51a `classify_disks` primitive, and on contact
  the producer pushes one `Occluder { target: Planet(back), front_dir,
  front_radius, kind, obscuration }` into the bounded list. Off-event
  the inner double loop costs 21 dot products and pushes zero entries,
  inside the analytic-mask "zero cost off-event" contract.
- `find_mutual_planetary_occultation(observer, planet_a, planet_b,
  start_jd_utc, end_jd_utc) -> Option<MutualPlanetaryOccultationEvent>`
  mirrors `find_lunar_occultation`: it rejects same-planet pairs up
  front, drives a 1-minute scan to locate the closest approach (and
  re-evaluates the front / back assignment at peak), then refines
  P1–P4 via the shared `contact_times` bisection. The event carries
  `front`, `back`, `kind`, `min_separation_rad`, `peak_obscuration`,
  `peak_jd_utc`, and `contacts`.

Validation (pinned in `VALIDATION.md`):

- `planning::tests::find_mutual_planetary_occultation_rejects_same_planet`
  guards the degenerate self-pair contract.
- `planning::tests::find_mutual_planetary_occultation_returns_none_off_event`
  asserts no false-positive event detection across Venus-Jupiter,
  Mercury-Mars, and Mars-Saturn on a quiet day (2025-07-01 Tokyo).
- `planning::tests::active_occluders_emit_no_planet_on_planet_off_event`
  pins the producer contract: on a normal day no occluder carries a
  `Planet(_)` target with a non-lunar front-disk radius. V-51d
  Moon-on-Planet entries (front radius = lunar apparent radius) are
  discriminated by the front-disk radius, two orders of magnitude
  larger than any planet's apparent radius.

Documented limit. Historical-event positive-detection validation
against the next visible mutual planetary occultation (2065-11-22
Venus occults Jupiter) is deferred until the DE440 upgrade tracked as
`L-06` lands; the current VSOP87 stack drifts a few minutes at that
epoch, which is acceptable for the producer contract but not for
sub-30 s P1–P4 matching against the historical canon.

### Unified eclipse / occultation pass (`V-51a` + `V-51b` + `V-51c` + `V-51d` + `V-51e` + `V-51f`)

All six slices of `V-51` (unified eclipse / occultation pass) have
shipped: common occultation primitives (`V-51a`), the general
`MAX_OCCLUDERS = 16` analytic-mask uniform array (`V-51b`), the
solar-eclipse renderer wiring (Moon → Sun pair, `V-51c`), lunar
occultation of stars and planets (`V-51d`), Mercury / Venus transit
of the Sun (`V-51e`), and mutual planetary occultation (`V-51f`). The
V-51f slice is documented in its own section above; the entry below
covers the V-51a / V-51b / V-51c primitive + uniform + renderer
foundation that the other slices build on.

Primary implementation areas:

- `crates/astronomy/src/occultation.rs` (new)
- `crates/astronomy/src/planning.rs` (eclipse search + state helpers)
- `crates/astronomy/src/ephemeris.rs` (V-36 lunar-eclipse aid folded
  into the V-51a `obscuration_fraction` helper)
- `crates/renderer/src/camera.rs` (`solar_eclipse_state` uniform)
- `crates/renderer/src/shaders/skyglow.wgsl` (analytic-mask Sun
  subtract, Koomen 1952 daylight falloff, Baumbach 1937 corona)
- `crates/common/src/presets.rs` + `docs/presets/sessions/solar-eclipse.json`
  (`SolarEclipse` deterministic preset wired to the 2024-04-08
  Mazatlán totality)

Shipped capabilities (`V-51a`):

- `ApparentDisk { direction, angular_radius_rad }` is the renderer /
  planning contract for any pair-wise occlusion; a unit direction in
  any consistent frame paired with an apparent semidiameter.
- `classify_disks(front, back) -> OccultationKind` returns
  `None | Partial | AnnularOrTransit | Total` from the Meeus AA §54
  apparent-disk geometry.
- `obscuration_fraction(front, back) -> f32` is the closed-form
  two-circle lens-area formula divided by the back-disk area, with
  the annular and total saturation edges handled in closed form.
- `contact_times(start, end, disks)` returns the four canonical
  contact instants P1–P4 via a 30 s grid scan + bisection refine
  (≤30 iterations, sub-50 ms numerical precision).
- The V-36 lunar-eclipse aid in `apparent_moon` now delegates its
  Earth-shadow fraction to `obscuration_fraction(Earth umbra, Moon)`,
  collapsing two copies of the same geometry into one.

Shipped capabilities (`V-51c`):

- `solar_eclipse_state(observer)` folds the Moon-on-Sun pair into a
  renderer-facing `(SolarEclipseKind, obscuration)` state, cheap
  enough to call every frame.
- `find_solar_eclipse(observer, start, end) -> Option<SolarEclipseEvent>`
  is the planning-side entry point: scans the window for peak
  obscuration, then refines P1–P4 via `contact_times`.
- `CameraUniform::solar_eclipse_state` is the GPU-side handle
  `[kind_code, obscuration, totality_weight, partial_weight]`. The
  renderer disables it on external galactic viewpoints and on
  `Atmosphere::OFF`.
- `shaders/skyglow.wgsl` extends `sun_moon_disk_radiance` with an
  analytic Moon-on-Sun subtract (no depth / stencil pass) plus a
  Baumbach 1937 corona term gated by the totality weight and
  evaluated only inside a 2° scissor centred on the Sun. The
  Hošek-Wilkie daylight and twilight branches multiply through a
  Koomen 1952 falloff that drops to ≈1e-4 of normal daylight at
  totality, with a continuous smoothstep through C2 / C3.
- New `SolarEclipse` scene preset framing the 2024-04-08 totality at
  Mazatlán (greatest eclipse, az≊138°, alt≊70°, 5° FoV), wired
  into the deterministic preset list and the validation gallery.

Validation (pinned in `VALIDATION.md`):

- `occultation::tests::*` pin the closed-form geometry for disjoint,
  touching, concentric total, concentric annular, point-source, and
  contact-time cases.
- `planning::tests::find_solar_eclipse_finds_2024_mazatlan_totality`
  asserts `Total` with `peak_obscuration > 0.999` and a 1–10 min
  totality duration against the Espenak / NASA TP-2006-214141 canon.
- `planning::tests::find_solar_eclipse_finds_2012_tokyo_annular`
  asserts the 2012-05-21 Tokyo annular eclipse and that the P1 /
  P4 contacts straddle the peak instant.
- `planning::tests::find_solar_eclipse_returns_none_on_non_eclipse_day`
  guards against false-positive event detection.
- `docs/assets/validation/solar-eclipse.png` ships in the deterministic
  gallery as the visual contract for the analytic-mask + Koomen +
  corona pipeline.

Shipped capabilities (`V-51b`):

- `astronomy::active_occluders(observer) -> ActiveOccluders` is the
  producer the renderer reads each frame to populate its bounded
  analytic-mask uniform. The list is a fixed-size, alloc-free
  container (`MAX_OCCLUDERS = 16`) so embedders can memcpy it into
  the uniform without an intermediate heap step. V-51b emits one
  entry (Moon → Sun) when `solar_eclipse_state` reports an event;
  V-51d/e/f extend the producer with their own front-disk pairs
  without further shader changes.
- `OccluderTarget { Sun, Moon, Planet(i), Stars }` routes the
  subtract mask to the correct back-disk source term (planet index
  matches `planet_eq_radius[i]`); the `Stars` variant flags the
  dormant CPU star-sprite cull tracked for V-51d.
- `CameraUniform::occluders: [[f32; 4]; 32]` + `occluder_params: [f32; 4]`
  carry the active list to the shader as two `vec4` rows per entry
  (`front_dir_radius` + `target_kind_obscuration`) plus a count
  header. Front-disk directions go through the same
  `apparent_disk_direction_j2000` pipeline as the Sun and Moon
  uniforms so the analytic mask stays bit-identical to V-51c.
- `shaders/skyglow.wgsl::occluder_subtract_mask(ray_dir, target_code,
  pixel_sr)` is the shared union-of-disks helper consumed by both
  the Sun and Moon disk source terms inside
  `sun_moon_disk_radiance`. The WGSL loop is bounded by
  `occluder_params.x` so padded rows are never sampled; outside an
  eclipse the count is zero and the shader short-circuits.
- Renderer parity tests
  (`camera::tests::occluder_uniform_matches_moon_state_at_mazatlan_peak`,
  `occluder_uniform_zeros_on_external_or_atmosphere_off`,
  `occluder_uniform_empty_off_eclipse`) pin the contract that V-51c
  golden frames remain bit-identical and that the list is gated by
  the same predicate as `solar_eclipse_state`. The committed
  `docs/assets/validation/solar-eclipse.png` (rendered through the
  array path) is the visual contract.
- `astronomy::planning::active_occluders_match_solar_eclipse_state_at_mazatlan_peak`
  pins the producer side: at greatest eclipse the list contains
  exactly one Sun-targeted occluder whose obscuration agrees with
  `solar_eclipse_state` to within 1e-6.

Deliberately out of scope for this slice (deferred to `V-51e`/`f`):
Mercury / Venus transit of the Sun and mutual planetary occultation.
The `MAX_OCCLUDERS = 16` rails and the `OccluderTarget` enum are in
place, so each follow-up item only needs to add its producer.

Documented limit. With the current VSOP87 / ELP2000 + WGS84
parallax ephemeris stack the apparent Moon and Sun radii at the
2024-04-08 Mazatlán peak come in within a few ×10⁻⁴ of equality;
`classify_disks` correctly returns `Total` once the search has
located the deepest sample. Sub-30-second P1–P4 accuracy against
NASA TP-2006-214141 requires the DE440 upgrade tracked as `L-06`;
the current contact times agree to within a few minutes, which is
adequate for the analytic-mask + Koomen + corona contract.

### Lunar occultation of stars and planets (`V-51d`)

Fourth slice of `V-51`: the Moon now occults catalog stars and the
seven rendered planets when its apparent disk falls in front of them.
The V-51b general `MAX_OCCLUDERS = 16` analytic-mask array and the
`OccluderTarget::{Stars, Planet}` codes were already in place, so this
slice is the producer + the two consumer hookups (planet disk
subtract + star vertex cull).

Primary implementation areas:

- `crates/astronomy/src/planning.rs` (`active_occluders` extended to
  emit Moon → Stars + Moon → Planet entries; new
  `find_lunar_occultation` planning helper with `LunarOccultedBody`
  and `LunarOccultationEvent` types).
- `crates/renderer/src/shaders/skyglow.wgsl` (`planet_disk_radiance`
  multiplies each per-planet contribution by `(1 - occluder_subtract_mask(...))`
  for the matching `OCCLUDER_TARGET_PLANET_BASE + i` target).
- `crates/renderer/src/shaders/star.wgsl` (vertex stage iterates the
  `OCCLUDER_TARGET_STARS = -1` entries and collapses occluded sprites
  to a degenerate clip-space quad behind the camera so the rasterizer
  drops the primitive before the fragment stage runs).

Shipped capabilities:

- `active_occluders` now emits up to nine entries per frame
  (Moon → Sun + Moon → Stars + Moon → each of the 7 planets when in
  contact). The Moon → Stars cull entry is emitted *unconditionally*
  so the star vertex shader can hide sprites behind the Moon every
  frame; off-occultation frames stay bit-identical to the pre-V-51d
  render because no catalog star sits inside the lunar disk except
  during an actual event. Moon → Planet entries are emitted only
  when the pair classifies as non-`None`, keeping the analytic-mask
  cost at zero off-event.
- `find_lunar_occultation(observer, body, start, end) ->
  Option<LunarOccultationEvent>` is the planning-side entry point.
  `body` is `LunarOccultedBody::{ Star { dir_date_eq }, Planet(p) }`;
  the helper drives a 1-minute scan to locate the closest approach
  and refines P1–P4 via the V-51a `contact_times` bisection.
- Star vertex shader cull: one normalised dot product + one
  `cos(radius)` comparison per active occluder per star vertex. Off
  any lunar occultation that’s one dot product per visible star per
  frame (the count = 1 Stars cull entry is always present); well
  inside the V-51 “no measurable fps regression” contract.
- Planet disk shader subtract: identical to the V-51c Sun path, just
  routed to the planet target codes. Mutual planetary occlusion is
  still gated on V-51f (which will plug a planet front-disk producer
  into the same uniform).

Validation (pinned in `VALIDATION.md`):

- `planning::tests::find_lunar_occultation_returns_none_off_event`
  guards against false-positive detection.
- `planning::tests::find_lunar_occultation_detects_synthetic_point_source`
  drives the helper with a fixed point source aligned to the Sun at
  the 2024-04-08 Mazatlán totality so the Moon disk covers it across
  the central phase, then pins central classification, contact-time
  bracketing, and the closest-approach geometry against the lunar
  apparent radius.
- `planning::tests::active_occluders_off_eclipse_emits_only_moon_on_stars`
  pins the producer side: off any solar / planet event the list
  contains exactly the one always-on `OccluderTarget::Stars` entry,
  with the lunar apparent disk as its front.
- `camera::tests::occluder_uniform_off_eclipse_emits_only_moon_on_stars`
  pins the renderer-side counterpart: the uniform carries exactly
  one entry (target code `-1`) with the lunar apparent radius.
- `camera::tests::occluder_uniform_matches_moon_state_at_mazatlan_peak`
  was extended to assert both the V-51c Sun entry (slot 0) *and* the
  V-51d Stars entry (slot 1) at greatest eclipse.

Deliberately out of scope for this slice (deferred to `V-51f`):
mutual planetary occultation. Sub-second IOTA contact-time validation
against published occultation predictions stays gated on `L-06` (the
DE440 ephemeris upgrade); the current VSOP87 / ELP2000 stack pins
detection and classification but not microsecond IOTA accuracy.

### Mercury / Venus transit of the Sun (`V-51e`)

Fifth slice of `V-51`: Mercury or Venus now occults the Sun when its
apparent disk crosses the photosphere. The V-51b general
`MAX_OCCLUDERS = 16` analytic-mask array routes the planet-disk
subtract through the same `OccluderTarget::Sun` slot the V-51c
Moon-on-Sun pair already uses, so the slice is the producer + the
planning helper without any shader change.

Primary implementation areas:

- `crates/astronomy/src/planning.rs` (`active_occluders` extended to
  emit `Planet → Sun` entries for Mercury and Venus when the inner
  planet is closer than the Sun and the disks are in contact; new
  `find_planet_transit` planning helper with `PlanetTransitEvent`).
- `crates/common/src/presets.rs` + `docs/presets/sessions/venus-transit.json`
  (`VenusTransit` deterministic preset wired to the 2012-06-06
  greatest transit from Tokyo).

Shipped capabilities:

- `active_occluders` emits a `Planet → Sun` entry whose front disk is
  the planet's apparent topocentric semidiameter, whose target is
  `OccluderTarget::Sun`, and whose kind is the raw
  `classify_disks(planet, sun)` label. The producer gates on
  `planet.distance_au < sun.distance_au` so the pure-geometry
  classifier rejects superior-conjunction near-alignments where the
  planet is in fact behind the Sun.
- `find_planet_transit(observer, planet, start, end) ->
  Option<PlanetTransitEvent>` mirrors `find_solar_eclipse`: a
  5-minute scan locates the peak and `contact_times` refines P1–P4
  via the shared bisection. The helper rejects outer planets up
  front (only Mercury and Venus can transit the Sun from Earth) and
  applies the same foreground gate inside the peak scan.
- Renderer side: zero new code. The V-51b shader path
  (`occluder_subtract_mask(OCCLUDER_TARGET_SUN, …)` inside
  `sun_moon_disk_radiance`) already iterates every active occluder
  whose target is the Sun, so the new planet entry draws as a black
  silhouette inside the solar sprite. The Koomen 1952 daylight
  falloff and Baumbach 1937 corona stay gated on
  `solar_eclipse_state`, which is computed from the Moon-on-Sun pair
  only — a transit therefore leaves the daylight sky untouched and
  does not light up the corona.
- New `VenusTransit` scene preset (2012-06-06T01:29:00Z, Tokyo, az
  113°, alt 55°, 2° FoV) and the matching
  `docs/presets/sessions/venus-transit.json` artifact wire the only
  Venus transit in the validation canon until 2117 into the
  deterministic preset list.

Validation (pinned in `VALIDATION.md`):

- `planning::tests::find_planet_transit_rejects_outer_planets`
  guards the foreground-planet gate (Mars / Jupiter must be rejected
  without running the scan).
- `planning::tests::find_planet_transit_returns_none_off_transit_day`
  guards against false-positive detection on a non-transit day.
- `planning::tests::find_planet_transit_finds_2012_venus_transit`
  asserts interior phase (P2/P3 present), peak obscuration in the
  area-ratio band (5e-4..2e-3), and a 5–8 h total duration against
  the NASA / IOTA canon.
- `planning::tests::active_occluders_emit_planet_on_sun_at_venus_transit_peak`
  pins the producer side: at greatest transit the list contains
  exactly one Sun-targeted occluder whose front radius matches the
  Venus apparent semidiameter and whose kind is
  `AnnularOrTransit`.
- `planning::tests::active_occluders_skip_planet_on_sun_at_superior_conjunction`
  pins the foreground gate at the 2024-06-14 Mercury superior
  conjunction: no Sun-targeted occluder is emitted even though the
  apparent directions overlap.

Deliberately out of scope for this slice (deferred to `V-51f`):
mutual planetary occultation (planet-on-planet). The `MAX_OCCLUDERS`
rails and the `OccluderTarget::Planet(i)` codes are already in
place; V-51f only needs to add a planet-front-disk producer that
routes to a planet target.

Note on the V-51 performance contract. The ROADMAP originally
phrased the star occlusion as a “CPU-side cull” (`10⁴ × 10 ≈ 0.1 ms`
in the planner block). The star-instance buffer is uploaded once at
renderer construction, so a per-frame CPU re-upload would force a
~2 MB GPU write each frame just to flip a handful of bits. The
shader-side discard runs in the same vertex pass that already reads
`corrected_j2000`, costs one dot + one compare per visible star per
active occluder, and matches the same “no measurable fps regression”
outcome the CPU formulation aimed for. The ROADMAP entry is updated
to reflect this in the same PR.

### Solar / lunar illuminants and physical sky colour

Implemented:

- solar irradiance / daylight-basis style illuminant values;
- lunar phase photometry for moonlight;
- Rayleigh / Mie / ozone-inspired daylight sky model;
- twilight radiance model continuous across civil, nautical, and astronomical
  bands;
- additive composition of sunlit sky, moonlit sky, and dark-sky background;
- atmosphere controls for turbidity, observer altitude, ozone, visibility,
  pressure, and temperature.

Primary implementation areas:

- `crates/astronomy/src/illuminants.rs`
- `crates/astronomy/src/atmosphere.rs`
- `crates/astronomy/src/skyglow.rs`
- `crates/renderer/src/camera.rs`
- `crates/renderer/src/skyglow.rs`
- `apps/cli`, `apps/viewer`, `apps/web`

### Planning UI

Implemented web planning helpers and UI for:

- local-evening rise / transit / set table;
- Sun, Moon, and planet planning objects;
- civil / nautical / astronomical twilight indicators;
- draggable web status-bar date / time controls, stepping by one local day or
  ten minutes respectively;
- session URLs using plain query parameters, with no version gate.

Primary implementation areas:

- `crates/astronomy/src/planning.rs`
- `apps/web/frontend`

### Observation-planning polish (`L-09`)

Layered Moon-impact and visibility scoring, recommended-object ranking,
favourites, and calendar export on top of the existing rise/transit/set and
twilight helpers.

- **Moon-impact (Krisciunas & Schaefer 1991, PASP 103, 1033).**
  `moon_sky_brightness_nanolamberts` implements their Eq. 15 from the
  Moon illuminance outside the atmosphere (Eq. 20), the Rayleigh + Mie
  scattering function `f(ρ)` (Eqs. 16–18), and the relative airmass
  `X(Z) = (1 − 0.96 sin²Z)^(−1/2)` (Eq. 3). `nanolamberts_from_v_mag` /
  `v_mag_from_nanolamberts` carry their Eq. 27 V-band surface-brightness
  conversion. `moon_impact` returns the ΔV degradation of a target's sky
  against a Moon-free baseline.
- **Visibility score.** `visibility_score` samples the evening window at
  5-minute steps and composes a unit-range product of an altitude term
  (saturating at 60°), a dark-window term (observable hours below the
  −12° Sun-depression threshold, saturating at 4 h), and a Moon-clarity
  term `10^(−0.4·ΔV)`. `rank_targets` / `planning_targets_from_bodies`
  produce "tonight's recommended objects"; `icalendar_for_targets`
  serialises each target's observable dark window to an RFC 5545 `.ics`
  document.
- **Site coupling.** The Moon-free baseline reads the `V-39`
  light-pollution zenith brightness
  (`LightPollution::zenith_sqm_mag_per_arcsec2`), so the scores reflect
  the configured Bortle / SQM site.
- **Hosts.** CLI `--plan-json` / `--plan-ical <path>` export the ranked
  plan and calendar without rendering. The web `StarView` bridge gains
  `planning_recommended_json` / `planning_ical`; the Planning settings
  card shows the ranking with score, max altitude, and Moon ΔV, a
  localStorage-backed favourites star toggle, and an "Export .ics"
  download button.
- **Tests.** Pinned K-S reference ΔV ≈ 2.99 mag/arcsec² for the
  ROADMAP case (target 20°, Moon 60°, 90 % illuminated), phase /
  separation monotonicity, V-mag ↔ nanolambert round-trip, unit-range
  and descending-rank scoring, iCalendar well-formedness, and a pinned
  JD → UTC timestamp.

Primary implementation areas:

- `crates/astronomy/src/planning.rs`
- `apps/cli/src/main.rs`
- `apps/web/src/lib.rs`, `apps/web/frontend`

## Reproducibility and platform baseline (legacy `Phase 3`)

### Schema-versioned JSON sessions

Implemented portable JSON sessions with an explicit `schemaVersion` and host
version metadata. Sessions preserve enough state to reproduce a render across
hosts:

- observer latitude / longitude;
- UTC, UT1, TAI, TT, approximate TDB, leap-second offset, and DUT1 fields;
- view azimuth / altitude / field of view;
- overlays, grid step, and opacity;
- projection, viewpoint, custom external camera vectors, and eyepiece optics;
- atmosphere preset plus all exposed atmosphere / refraction controls;
- planet visibility, active correction flags, and catalog snapshot metadata.

Native hosts can load sessions with `--session`; the CLI can also write the
effective scene with `--write-session`. The web settings panel can copy/download
and load the same JSON shape, while the existing compact URL format remains
available for quick sharing.

Primary implementation areas:

- `crates/common/src/session.rs`
- `apps/cli/src/main.rs`
- `apps/viewer/src/main.rs`
- `apps/web/frontend/src/session.ts`
- `apps/web/frontend/src/components/StatusBar.tsx`

### Data provenance manifest

Implemented `L-15` (legacy `P3-13`): a machine-readable manifest that records every committed
data artifact, every regenerable artifact, and every runtime web service the
application calls.

Shipped capabilities:

- `data/manifest.toml` enumerates 34 local artifacts (HYG v4.2, d3-celestial
  constellation lines, IAU/Delporte boundaries, 13 scene preset JSONs, 2
  notebook expected CSVs, 3 README gallery PNGs, 13 validation gallery PNGs)
  and the OpenStreetMap Nominatim runtime endpoint, each with SHA-256, source,
  license, version, preprocessing command, and field list.
- `crates/manifest` (`stars-manifest`) parses and validates the TOML schema,
  enforces per-`kind` required fields (`embedded` / `generated` /
  `runtime-service`), and exposes a `verify_artifact` API that re-hashes the
  bytes at the recorded `path`. Other crates can resolve manifest ids to a
  pinned `(path, sha256, source)` tuple.
- A `check-manifest` binary (`make manifest-check`, wired into `make ci`)
  walks every artifact and fails on missing files, hash drift, or byte-size
  drift. Editing a data file without updating its `sha256` in the same PR is
  now a CI failure.
- `DATA_SOURCES.md`, `CONTRIBUTING.md`, `AGENTS.md`, and `ARCHITECTURE.md`
  are updated to point at the live manifest. The catalog backend design
  already calls for manifest references; the manifest now provides the
  stable artifact ids those references will use.

Validation:

- `crates/manifest` unit tests pin the schema (duplicate ids, missing `path`
  on `embedded`, missing `preprocessing` on `generated`, runtime-service with
  a forbidden `path`, future schema version);
- `repository_manifest` integration test loads the real `data/manifest.toml`,
  walks every entry, and asserts SHA-256s match the committed bytes. This
  runs under `cargo test --workspace` so unaccounted-for data drift surfaces
  even without invoking `make manifest-check` directly.

Primary implementation areas:

- `crates/manifest/src/lib.rs`
- `crates/manifest/src/bin/check-manifest.rs`
- `crates/manifest/tests/repository_manifest.rs`
- `data/manifest.toml`
- `Makefile` (`manifest-check` target wired into `ci`)

### Citation and standards baseline

Implemented the first citable-platform baseline for the Library track:

- `CITATION.cff` provides repository-level software citation metadata and
  preferred citation guidance for teaching, publications, validation reports,
  and derivative software.
- `.zenodo.json` records release-archive metadata so tagged GitHub releases can
  be deposited in Zenodo and cited with version-specific DOIs once minted.
- `docs/citation.md` gives the preferred citation text, Zenodo release
  checklist, and caveats that must accompany scientific figures: code identity,
  JSON session, catalog/data identity, model limits, and rendering limits.
- `docs/standards-compliance.md` lists implemented IAU/SOFA-aligned constants
  and routines, renderer-grade approximations, and deliberate non-goals.

Validation:

- metadata files are text/JSON/YAML and are syntax-checked as part of this
  documentation-only change;
- the standards page cross-references the implementation files that contain the
  pinned numerical tests.

### Catalog backend scaling scaffold

Implemented the `L-16` (legacy `P3-00`) catalog scaling seam before large catalog ingest. The
current product remains HYG-backed, but future Hipparcos / Tycho-2 / Gaia DR3
work now has explicit API and documentation boundaries.

Shipped capabilities:

- `CatalogBackend` trait with `CatalogQuery`, `CatalogPage`, and
  `CatalogSource` metadata;
- stable backend names for HYG CSV and embedded HYG sources;
- CPU-side `CatalogIdentifiers` / `CatalogObjectId` fields on `Star` rows for
  HYG, HIP, HD, Tycho-2, and Gaia DR3-style numeric IDs;
- `HygCsvBackend` adapter used by the existing filesystem `load_from_file`
  compatibility helper, plus `HygEmbeddedBackend` for compact embedded builds;
- source-side magnitude / row-limit query shape, with paging fields reserved for
  larger LOD backends;
- `docs/catalog-backend-design.md` covering identifier policy, LOD / spatial
  index strategy, streaming / paging, and WASM subset constraints.

Primary implementation areas:

- `crates/catalog/src/backend.rs`;
- `crates/catalog/src/catalog.rs`;
- `docs/catalog-backend-design.md`;
- `ARCHITECTURE.md`.

Validation:

- catalog tests pin backend source labels, identifier preservation, and HYG
  query filtering / truncation;
- native session snapshot helpers now derive HYG backend/source/version strings
  from `CatalogSource::HYG_CSV`.

### Scene presets, notebook examples, and validation gallery

Implemented deterministic Library-track scene presets for reproducible demos,
validation screenshots, notebooks, and bug reports. Native hosts can list or
load presets with `--list-presets` / `--preset`; the CLI can export any
effective preset or scene as JSON with `--write-session --write-session-only`.

Preset coverage includes:

- Tokyo evening and high-altitude dark sky;
- noon, sunset, civil twilight, nautical twilight, and astronomical twilight;
- moonlit night and a lunar-eclipse aid scene;
- Hammer and Mollweide all-sky maps;
- built-in galactic-north and custom external galactic viewpoints.

The notebook workflow in `examples/notebooks` loads the same JSON sessions,
uses the `stars-cli` `session-table` example to produce tabular Sun/Moon/planet
outputs, compares those outputs with committed CSV fixtures, and can render the
same scene through the CLI without requiring Python bindings. The validation
gallery workflow renders presets to `docs/assets/validation/` and optionally
compares regenerated PNGs against committed baselines when the rendering adapter
is stable enough for screenshot CI.

Primary implementation areas:

- `crates/common/src/presets.rs`;
- native `--preset`, `--list-presets`, and CLI `--write-session-only` wiring in
  `apps/cli` and `apps/viewer`;
- `scripts/export-scene-presets.sh`;
- `scripts/render-validation-gallery.sh`;
- `docs/scene-presets.md`;
- `docs/validation-gallery.md`;
- `apps/cli/examples/session_table.rs`;
- `examples/notebooks`.

Validation:

- host-common tests pin preset metadata uniqueness and JSON session
  round-trips;
- `make notebook-check` compares notebook astronomy tables with pinned CSV
  fixtures without requiring Jupyter, a star catalog, or a GPU;
- the gallery script provides repeatable human screenshots and opt-in exact
  screenshot regression for pinned GPU/driver environments.

## Advanced visual features (legacy `Phase 4`)

### Deep-sky overlay (Messier + bright NGC / IC subset)

`V-42` (legacy `P4-03`) now ships in two layers: the original Messier slice
plus a bright NGC / IC subset extracted from OpenNGC, controlled by the
same density slider. A trait-based `catalog::deepsky` API replaces the
renderer-internal decoder so the runtime full-OpenNGC streaming backend
planned as the next PR slots in without further renderer churn.

Shipped capabilities:

- `crates/catalog/data/messier.csv` (110 Messier objects, unchanged data,
  moved from `crates/renderer/data/`) and the new
  `crates/catalog/data/openngc_bright.csv` (~1,250 NGC / IC objects at
  V ≤ 11.5 mag plus large diffuse nebulae lacking integrated photometry,
  produced deterministically by `scripts/extract-openngc-bright.py` from
  the upstream OpenNGC snapshot).
- `catalog::deepsky::DeepSkyCatalog` trait with two embedded
  implementations (`MessierCatalog`, `NgcBrightCatalog`) and a shared
  `DeepSkyObject` / `DeepSkyId` ADT covering Messier, NGC, and IC primary
  identifiers. The catalog crate's build script compacts both CSVs into
  the i16 binaries `messier.bin` and `openngc_bright.bin`.
- `OverlayKind::DeepSkyObjects` now renders Messier objects as the
  existing 4-segment diamond and NGC / IC objects as a distinct 8-segment
  ring, so the user reads the catalogue at a glance without needing the
  label. Marker sizes share the same arcminute clamp
  (`DEEP_SKY_MARKER_MIN_ARCMIN` / `DEEP_SKY_MARKER_MAX_ARCMIN`).
- `OverlayKind::DeepSkyLabels` ingests Messier and NGC / IC labels in one
  pass and tints them slightly differently (warmer green for Messier,
  cooler teal for NGC / IC) to mirror the marker-shape distinction.
- Famous diffuse anchors that OpenNGC publishes without integrated
  photometry (NGC 7000 North America Nebula, NGC 1499 California Nebula,
  NGC 2237 Rosette Nebula, IC 405 Flaming Star, …) store a sentinel
  magnitude (99.00) so they remain hidden behind the default density
  slider until the user opens it past the sentinel.
- `scripts/extract-openngc-bright.py` is the deterministic regenerator;
  re-running it against a cached OpenNGC snapshot produces byte-stable
  output, supporting the manifest `sha256` discipline.
- `data/manifest.toml` records the relocated `messier-catalog` artifact at
  its new catalog-crate path and adds `openngc-bright-catalog` with the
  extraction script as `preprocessing`.

Validation:

- `catalog::deepsky` unit tests pin: every Messier number 1..=110 present
  exactly once; M31 / M42 / M45 J2000 spot checks and classifications;
  i16 magnitude / size quantisation round-trips; absence of Messier IDs
  in the NGC bright table; anchor coverage for NGC 7000 / 253 / 869 / 884 /
  7293 / 5128 / 1499 / 6960 / IC 434; the no-photometry sentinel filter
  policy; and inclusive-magnitude filter behaviour.
- `renderer::overlay` updated tests pin the Messier diamond contribution
  (110 × 8 vertices) plus a multiple-of-16 NGC ring contribution at the
  show-all magnitude limit, NaN-safe gating, and slider monotonicity
  (limit = -10 → empty; default cutoff strictly less than show-all).
- `renderer::text` updated tests assert that the deep-sky label table
  contains exactly 110 Messier entries plus the canonical NGC 7000 and
  IC 434 anchors.
- `stars-manifest` integration tests re-verify both `messier-catalog` and
  `openngc-bright-catalog` SHA-256 hashes against the on-disk CSVs.

Documented limitations:

- The committed NGC bright subset deliberately drops OpenNGC entries
  marked `Dup` (which removes NGC 2244 Rosette cluster, IC 2118 Witch Head)
  and large diffuse objects whose published `MajAx` falls below the
  30-arcmin threshold despite being visually huge (IC 1396 records 14′
  upstream). The planned runtime streaming backend will expose those
  entries from the full OpenNGC catalogue.
- Magnitudes for extended objects are single-value approximations;
  treat the values as catalogue-grade ordering, not photometric truth.

Primary implementation areas:

- `crates/catalog/data/{messier.csv,openngc_bright.csv}`
- `crates/catalog/build.rs`
- `crates/catalog/src/deepsky.rs`
- `crates/catalog/src/lib.rs`
- `crates/renderer/build.rs` (label generation only; binaries moved)
- `crates/renderer/Cargo.toml` (new `catalog` dependency)
- `crates/renderer/src/lib.rs` (drop `mod deepsky`)
- `crates/renderer/src/overlay.rs` (consume `MessierCatalog` /
  `NgcBrightCatalog`; ring marker added)
- `crates/renderer/src/text.rs` (`DEEP_SKY_LABELS` replaces
  `MESSIER_LABELS`)
- `scripts/extract-openngc-bright.py`
- `data/manifest.toml`

Follow-up: the row in `ROADMAP.md` notes that the runtime streaming
backend (`OpenNgcCsvCatalog`) for the full ~14,000-entry catalogue is the
next PR. Identifier preservation through the renderer (hover / click →
NGC / PGC IDs) tracks separately as `L-18`.

### Full-sky projections

Implemented the first niche-visual feature (`V-40`, legacy `P4-01`): selectable screen projections.
Perspective remains the default camera projection, and three all-sky map modes
are available for structure-scale views:

- Mollweide;
- Aitoff;
- Hammer.

The all-sky modes map the entire celestial sphere into a 2:1 ellipse fitted to
the framebuffer, keep azimuth / altitude as the map centre, and ignore the
perspective FoV slider. The skyglow / daylight / twilight pass reconstructs
rays through the inverse selected map projection, while stars and overlays use
the corresponding forward projection.

Primary implementation areas:

- `renderer::SkyProjection` and camera projection uniforms;
- `crates/renderer/src/shaders/star.wgsl`;
- `crates/renderer/src/shaders/skyglow.wgsl`;
- `crates/renderer/src/shaders/overlay.wgsl`;
- projection controls in `apps/cli`, `apps/viewer`, and `apps/web`.

Validation:

- renderer unit tests pin projection string round-trips, all-sky map fitting,
  and average all-sky pixel solid angle;
- host-common tests pin native CLI enum mapping;
- smoke renders cover perspective, Mollweide, Aitoff, and Hammer shader paths.

### Out-of-Earth galactic viewpoint

Implemented the external camera mode (`V-41`, legacy `P4-02`) for viewing the local Milky Way from
above the north galactic pole. The default Earth-centred view remains unchanged,
but hosts can now select `galactic-north` / `SkyViewpoint::GalacticNorth` to:

- move the camera off Earth into a parsec-scale IAU galactic Cartesian frame;
- place HYG catalogue stars by their stored parsec distances;
- skip atmosphere, refraction, and Earth-local overlays for the external map;
- render an analytic top-down Milky Way disc in the skyglow pass so local stars
  have galaxy-scale context.

Primary implementation areas:

- `renderer::SkyViewpoint` and camera viewpoint uniforms;
- HYG distance plumbing in `catalog::Star` and `renderer::StarInstance`;
- `crates/renderer/src/shaders/star.wgsl` for parsec-position projection;
- `crates/renderer/src/shaders/skyglow.wgsl` for the top-down disc context;
- viewpoint controls in `apps/cli`, `apps/viewer`, and `apps/web`.

Validation:

- catalog tests pin loaded distances;
- renderer tests pin viewpoint string round-trips and the external camera
  uniform;
- host-common tests pin native CLI enum mapping.

### Custom external viewpoint origin

Generalized the fixed galactic viewpoint (`V-44`, legacy `P4-07`) into a host-selectable external
camera. `SkyViewpoint::GalacticNorth` remains the preset top-down view, while
`SkyViewpoint::CustomExternal` uses `renderer::ExternalViewpoint` to carry an
origin, target, and up vector in IAU galactic Cartesian parsecs. The coordinate
frame is documented as Sun-centred, `+X` toward galactic longitude `l=0°`, `+Y`
toward `l=90°`, and `+Z` toward the north galactic pole.

Primary implementation areas:

- `renderer::ExternalViewpoint` and custom external camera matrix / uniforms;
- native `--external-origin-pc`, `--external-target-pc`, and `--external-up`
  flags for `apps/cli` and `apps/viewer`;
- WASM bindings plus web settings, persistence, and session URL parameters
  (`originPc`, `targetPc`, `up`);
- architecture and roadmap documentation for the external coordinate frame.

Validation:

- renderer tests pin custom external origin upload and finite camera matrices;
- host-common tests pin `custom-external` CLI round-trips and override
  selection;
- Rust workspace check and frontend TypeScript check cover host wiring.

### Galactic structural model for external viewpoints (`V-46`)

Upgraded the external galactic viewpoint (`V-41` / `V-44`) from a single
analytic Milky Way disc plane to a Drimmel & Spergel 2001 style multi-component
volume model, so the external view is a defensible educational graphic of the
Galaxy's structure rather than a generic ellipse.

- `crates/astronomy/src/galaxy.rs` is the reference implementation:
  `milky_way_luminosity_density(x, y, z)` sums a thin disk (sech² vertical
  profile) modulated by a Reid 2019 four-arm log-spiral enhancement
  (`spiral_arm_enhancement` / `SPIRAL_ARMS`), an exponential thick disk, and a
  rotated triaxial boxy bar/bulge; `dust_extinction_az(distance, l, b)`
  integrates a double-exponential dust disk along the line of sight.
  Galactocentric IAU parsecs with the Sun on `+x` at `R_SUN_PC = 8122`,
  `Z_SUN_PC = 20.8`.
- `crates/renderer/src/shaders/skyglow.wgsl` external-viewpoint branch now
  emission-absorption ray-marches `gal_density` attenuated by `gal_dust_density`
  instead of intersecting one plane, resolving the bar, spiral arms, and dark
  dust lanes. The WGSL constants mirror `astronomy::galaxy` value-for-value
  (kept in lock-step like the ISL model) and the pinned Rust tests guard the
  shared model.
- No new host control, session field, or `CameraUniform` slot: the existing
  external viewpoint toggle drives the upgrade across CLI / viewer / web. The
  model is closed-form, so no new data artifact / manifest row.

Validation:

- `astronomy::galaxy` pins the solar-neighbourhood density (≈ 0.1 M_sun/pc³),
  the fall-off with height and radius, the bar dominating the centre, the
  on-arm vs inter-arm contrast, and the dust screen's monotonicity /
  plane-concentration / ≈ 1 mag/kpc local gradient;
- the `skyglow.wgsl` change was validated through the naga WGSL front-end
  during development and is exercised by the existing renderer build.

### Telescope eyepiece simulation

Implemented the telescope eyepiece model (`V-43`, legacy `P4-06`). Hosts can enable an optical
train consisting of OTA aperture / focal length and eyepiece focal length /
apparent field / optional field stop. The renderer derives:

- focal-plane plate scale in arcseconds per millimetre;
- eyepiece magnification;
- exit-pupil diameter;
- true field of view, preferring the physical field stop and falling back to
  apparent field divided by magnification.

When enabled in the Earth-centred perspective view, the true field overrides the
regular camera FoV while retaining the same azimuth / altitude pointing. CLI and
desktop expose matching flags, and the web settings panel persists the optical
train and includes it in shareable session URLs.

Primary implementation areas:

- `renderer::EyepieceSimulation` and `Camera::effective_view`;
- native `--eyepiece`, `--telescope-aperture-mm`, `--telescope-focal-length-mm`,
  `--eyepiece-focal-length-mm`, `--eyepiece-apparent-fov-deg`, and
  `--eyepiece-field-stop-mm` flags for `apps/cli` and `apps/viewer`;
- WASM bindings plus web settings, localStorage, status display, and session URL
  parameters (`eyepiece`, `otaApertureMm`, `otaFocalMm`, `eyepieceFocalMm`,
  `eyepieceAfovDeg`, `eyepieceFieldStopMm`).

Validation:

- renderer tests pin plate scale, magnification, exit pupil, and true-FOV
  formulas;
- renderer tests pin that eyepiece FoV applies only to Earth perspective views;
- host-common tests pin native override / enable semantics;
- Rust workspace tests, WASM check, and frontend TypeScript check cover host
  wiring.

## Web UI internationalisation (English + Japanese)

The web frontend now ships a minimal, dependency-free i18n layer covering
English and Japanese. The same React build serves either locale; nothing in
the Rust engine or WASM bridge had to change.

Locale selection priority:

1. `?lang=en|ja` URL parameter (so shared session URLs can pin a language);
2. `navigator.language` / `navigator.languages` prefix match;
3. fallback to `en`.

There is no in-app language switcher: the UI follows the browser. Users who
want to override the auto-detected locale pass `?lang=ja` (or `?lang=en`) in
the URL or change their browser's preferred language. The active locale is
mirrored onto `<html lang="…">` so assistive tech sees the right language.
English remains the source of truth for the key set; missing Japanese keys
fall back to English at lookup time.

The canonical English strings emitted by the wasm planning bridge
(`PlanningBody::name`, `TwilightBand::label` — e.g. `"Mercury"`,
`"Civil twilight"`) stay in Rust; the JS side translates them via
`translateWasmBody` / `translateWasmTwilight` so the renderer stays
locale-agnostic.

Primary implementation areas:

- `apps/web/frontend/src/i18n.tsx` (context, provider, hook, dictionaries,
  WASM-string translation helpers);
- `apps/web/frontend/src/main.tsx` wraps the root in `<I18nProvider>`;
- `apps/web/frontend/src/App.tsx`,
  `apps/web/frontend/src/components/StatusBar.tsx`, and
  `apps/web/frontend/src/components/OverlayToggles.tsx` resolve every
  user-facing string through `useT()`;
- the legacy `OVERLAY_LABELS`, `ATMOSPHERE_PRESET_LABELS`,
  `SKY_PROJECTION_LABELS`, and `SKY_VIEWPOINT_LABELS` constants in
  `observer.ts` were removed in favour of `overlay.*`, `atmospherePreset.*`,
  `projection.*`, and `viewpoint.*` keys.

Validation:

- `make frontend-check` (TypeScript strict mode) covers the React refactor.
  No numerical behaviour changes, so no new pinned numerical tests were
  added.

## Web status-bar polish (single-line strip, draggable values, tabbed settings)

The web status strip and Settings popover were reworked for a less cluttered
look and faster interaction. No engine or numerical behaviour changed.

Status strip:

- One inline row instead of two stacked rows; the floating background panel
  was dropped in favour of a text-shadow so the strip sits directly over the
  sky.
- Every value on the strip is now a horizontal scrubber via the shared
  `useStepDrag` hook (`apps/web/frontend/src/components/useStepDrag.ts`):
  date (±1 day/step), clock (±10 min/step), latitude / longitude (±0.1°
  /step), azimuth (±1°/step), altitude (±0.5°/step), and FOV
  (multiplicative, factor 0.97/step, drag right = zoom in to match the
  scroll wheel). Az/Alt clamp / wrap through the existing `clampAltitude` /
  `wrapAzimuth` / `clampFov` helpers from `observer.ts`.
- The Projection text on the strip is now a button: clicking it cycles
  through `SKY_PROJECTIONS` in place, only when the viewpoint is Earth
  (matches the disabled-selector logic in the Settings panel).

Popovers (Location / Time / Settings):

- Click-anywhere-outside dismiss via a transparent fixed backdrop sitting
  behind the popover layer.
- Sticky header with the close button — the dialog title and `×` stay
  visible while the body scrolls.
- Settings popover stretches between the viewport top and the status strip
  (`top: 14; bottom: 56`) so long content keeps its top edge attached to
  the window instead of overflowing upward.
- Location popover replaces the lat/lng sliders with plain number inputs
  (precision 0.0001°). Time popover drops the redundant date picker since
  the local datetime input covers the same job.

Settings panel reorganisation:

- Tabbed view with four sections: **Sky** (solar-system toggle, overlays,
  planning), **View** (viewpoint, external camera, screen projection,
  telescope eyepiece), **Environment** (atmosphere & extinction), and
  **Session** (copy URL / copy JSON / load JSON).
- Flat section style: nested card-in-card boxes were removed; section
  headings now sit flush with the popover edge, separated only by parent
  grid `gap`. `OverlayToggles` was switched from `<fieldset>` groups to
  flat `<section>` blocks to match.

Language switcher removal:

- The Settings panel's English / 日本語 button row was removed; the locale
  is now detected once at load from `?lang=` or the browser preference (see
  the i18n section above). `localStorage` persistence was dropped together
  with `useState` / `setLocale` / `LOCALES` / `LOCALE_LABELS` exports.

Primary implementation areas:

- `apps/web/frontend/src/components/StatusBar.tsx` — strip layout,
  scrubber wiring, popover shell, sticky header, settings tabs;
- `apps/web/frontend/src/components/useStepDrag.ts` — new shared horizontal
  drag hook used by every scrubber;
- `apps/web/frontend/src/components/OverlayToggles.tsx` — flat sub-section
  styling matching the parent settings layout;
- `apps/web/frontend/src/i18n.tsx` — dropped manual locale switcher,
  added strings for new drag tooltips, projection-cycle hint, tab labels,
  and the new Solar-system card;
- `apps/web/frontend/src/App.tsx` — passes `setView` through as
  `onSetView` so the new scrubbers can mutate camera state.

Validation:

- `make frontend-check` (TypeScript strict mode) covers the refactor; no
  engine, renderer, or numerical behaviour changes.

## V-26 Lunar earthshine (Da Vinci glow)

The lunar disk shader now composes a Lambertian dark-side earthshine term
additively with the existing lit-side Lambertian, so crescent phases show
the characteristic faint glow on the unlit hemisphere ("Da Vinci glow",
ashen light) lit by sunlight reflected from Earth.

Shipped capabilities:

- `astronomy::illuminants::earthshine_disk_luminance_cd_m2(phase, earth_albedo,
  lunar_albedo)` returns the dark-side mean surface luminance in cd/m². The
  closed form is anchored to Goode et al. 2001 / Danjon 1936 dark-side
  photometry: with canonical Bond albedos (Earth 0.30, Moon 0.12) and
  `phase = 60°` the function returns V ≈ +13.7 mag/arcsec² (≈ 0.36 cd/m²),
  yielding a dark-to-full-Moon-lit surface-brightness ratio of order 10⁻³ at
  thin crescent phases. The roadmap's reference "V ≈ +3.7 mag/arcsec²" is
  the Danjon mag/arcmin² convention; the crate uses mag/arcsec² throughout,
  matching all the other V-band photometric paths and the unit-test
  ratio target.
- `shaders/skyglow.wgsl::earthshine_disk_luminance_cd_m2` is a literal port
  of the same closed form (f32) with canonical Bond albedos baked in. The
  lunar fragment composes `lit_side_lambertian + dark_side_earthshine`,
  reusing the existing near-hemisphere normal reconstruction in
  `lunar_phase_lambert` (with `sun_dir = -moon_dir` for Earth-illumination).
- Dark-side per-channel atmospheric extinction follows the same Schaefer
  1993 / Kasten-Young 1989 path the diffuse sky pass uses, so a low-
  altitude crescent attenuates its earthshine in lockstep with the rest
  of the scene (V-37).

Tests / validation:

- `astronomy::illuminants::tests::earthshine_monotonic_in_phase` pins
  `dark = 0` at full Moon and monotonic growth through crescent phases,
  plus the V = 13.7 mag/arcsec² anchor at α = 60°.
- `astronomy::illuminants::tests::earthshine_5pc_crescent_within_half_mag_of_reference`
  is the pinned 5%-crescent V-band check (±0.5 mag/arcsec² of V ≈ +12.2).
- `astronomy::illuminants::tests::earthshine_to_full_moon_ratio_is_order_milli`
  pins the dark/lit ratio to the 10⁻⁴–10⁻² band at the 5% crescent.
- `astronomy::illuminants::tests::earthshine_scales_linearly_in_both_albedos`
  pins the `α_Earth · α_Moon` linearity in the Lambertian double-reflection
  model.
- `renderer::lunar_phase::tests::shader_anchor_matches_astronomy_crate`
  pins the WGSL anchor constant against the astronomy crate's helper
  across a phase-angle sweep so the GPU value cannot silently drift.

Primary implementation areas:

- `crates/astronomy/src/illuminants.rs`
- `crates/renderer/src/shaders/skyglow.wgsl`
- `crates/renderer/src/lunar_phase.rs`

Hosts wired: CLI, viewer, web (single shader change, shared by every host).

References:

- Danjon, A. 1936, Ann. Obs. Strasbourg 3, 139 ("Photometric measurements
  of earthshine").
- Goode, P. R. et al. 2001, GRL 28, 1671 ("Earthshine observations of
  the Earth's reflectance").
- Qiu, J. et al. 2003, JGR 108, D22 (phase dependence and Bond-albedo
  retrieval).

## `V-25` Differential atmospheric dispersion

Wavelength-dependent refraction now renders horizon-near point sources as
short vertical R–G–B streaks (blue end higher, red end lower) and gives
the setting Sun / Moon a faintly reddened lower limb and bluer upper
limb. The renderer previously applied one altitude-only refraction value
to every channel; the dispersion is now baked into the PSF footprint and
the analytic Sun / Moon disk mask rather than added as a post-process
tint.

Shipped capabilities:

- `astronomy::corrections::refraction_per_wavelength(true_altitude_rad,
  pressure_hpa, temperature_c, wavelength_nm)` returns the refraction
  angle `ρ(λ) = apparent − true` in radians. The broadband Saemundsson
  apparent-altitude refraction is scaled by the Edlén 1966 refractivity
  ratio `(n(λ) − 1) / (n(550 nm) − 1)`, so the green channel matches the
  existing single-wavelength path bit-for-bit and the differential
  `ρ(B) − ρ(R)` follows the Edlén dispersion shape.
- `astronomy::RGB_REFERENCE_WAVELENGTHS_NM = [620, 550, 440]` and
  `EDLEN_REFERENCE_REFRACTIVITY` are now public so renderer and
  validation code share one source of truth for the R / G / B anchors.
- `crates/renderer/src/shaders/star.wgsl` projects three apparent
  directions per star (one per channel) and emits the green-relative
  pixel offsets for red and blue as a fourth vertex output
  (`dispersion_px_rb`). The fragment shader samples the radial Spencer
  PSF at the three offset centres and packs the chromatic intensities
  into the per-pixel `vec3` RGB output. The ciliary corona and edge
  apodization stay shared across channels because both are geometric
  (lens / sprite-window) effects.
- `crates/renderer/src/shaders/skyglow.wgsl` shifts the Sun and Moon
  disk centres per channel along the local-vertical great-circle by
  `ρ_total(alt) · (ratio_X − 1)`, then composes a per-channel disk mask.
  At high altitudes the three masks coincide and the result reduces to
  the legacy single-mask render; near the horizon the shifted masks
  produce the characteristic chromatic limbs.
- Pressure / temperature flow through the existing
  `Atmosphere::pressure_hpa` / `Atmosphere::temperature_c`
  (`V-34` controls). No new host parameters were introduced; the
  feature is automatically gated off when
  `Atmosphere::sunlit_scattering = false` (`Atmosphere::OFF` and the
  external galactic viewpoint).

Tests pinned:

- `crates/astronomy/src/corrections.rs::rgb_dispersion_at_five_degrees_is_arcsecond_scale`
  asserts `ρ(440 nm) − ρ(620 nm) ∈ [6″, 12″]` at altitude 5°, 1013 hPa,
  10 °C. The roadmap's original `[1.2″, 2.5″]` target was inconsistent
  with Edlén + Saemundsson at altitude 5° — the correct value is
  ≈8.8″, still firmly naked-eye-visible. The roadmap's qualitative
  criterion (“naked-eye visible on Sirius or the Sun’s lower limb”) is
  the one that ships; the numeric window has been widened to reflect
  the physics.
- `corrections::tests::rgb_dispersion_decreases_with_altitude` pins
  the monotone falloff with altitude (alt = 5° > 30° > 60°) and the
  blue-above-red sign.
- `corrections::tests::edlen_refractivity_brackets_550nm_with_rgb_anchors`
  asserts `n(620) < n(550) < n(440)` and pins the 550 nm reference
  constant.
- `renderer::camera::tests::rgb_dispersion_ratios_agree_with_astronomy`
  re-parses both `star.wgsl` and `skyglow.wgsl` to assert that each
  `DISPERSION_RATIO_{R,G,B}` constant equals the corresponding host
  Edlén ratio, so the renderer cannot silently drift away from the
  astronomy crate.

References:

- Filippenko, A. V. 1982, PASP 94, 715.
- Stone, R. C. 1996, PASP 108, 1051.
- Edlén, B. 1966, Metrologia 2, 71.
- Cox, A. N., ed. 2000, *Allen's Astrophysical Quantities*, §3.281.

Hosts wired: CLI / viewer / web (all consume the same renderer crate,
so the dispersion ships through the shared shader pipeline without
any host-side wiring changes).

### Belt of Venus and Earth-shadow band (V-27)

Closed the anti-solar gap in the twilight composition: the existing
V-33 twilight model is zenith-symmetric in luminance, so the pink Belt
of Venus arch and the blue-grey Earth-shadow band below it were not
rendered. V-27 adds compact `(relative_az, view_alt)` 2-axis fits to
Lee & Hernández-Andrés 2003 measurements of the anti-twilight arch and
shadow band, evaluated identically on the Rust side (for unit tests and
the documented model) and inside the WGSL twilight pass.

Scientific basis:

- Hulburt 1953 (JOSA 43, 113) explains the warm anti-twilight arch as
  Rayleigh-stripped, red-pass single-scattering along the long
  anti-solar slant column.
- Lee & Hernández-Andrés 2003 (Appl. Opt. 42, 445) measure the
  radiance and chromaticity field across solar depression and relative
  azimuth and supply the empirical envelope the fits are anchored to.
- Adams, Plass & Kattawar 1974 (J. Atmos. Sci. 31, 1662) provide the
  multiple-scattering context for the band darkening inside the
  geometric Earth shadow.

Shipped capabilities:

- `astronomy::atmosphere::antitwilight_arch_radiance(sun_alt, relative_az,
  view_alt)` returns a per-channel `[R, G, B]` multiplier on top of the
  V-33 zenith twilight reference. Peak amplitude
  `(R, G, B) = (+0.28, +0.04, -0.18)` is reached at relative azimuth
  180°, view altitude ≈ 8°, and solar depression ≈ 3°; the multiplier
  collapses to `[1, 1, 1]` outside the civil-twilight depression window
  `(0°, 6.5°)` so daylight and nautical/astronomical twilight pass
  through unchanged.
- `astronomy::atmosphere::earth_shadow_band_radiance(...)` returns the
  matching cool-band multiplier with peak amplitude
  `(R, G, B) = (-0.40, -0.32, -0.22)` at the anti-solar horizon, giving
  a blue-grey tint and a clear luminance dip in the band.
- `crates/renderer/src/shaders/skyglow.wgsl` evaluates the same two fits
  inside `twilight_sky_radiance`. Sun and view directions are projected
  onto the local horizon plane to get a stable relative azimuth even
  when the Sun is below the horizon, and the per-channel multipliers
  are applied before HDR conversion so the existing daylight ↔ twilight
  ↔ dark-sky composition stays additive in physical units.
- New deterministic scene preset `civil-twilight-antisolar-tokyo`
  (Tokyo, 2026-06-21 10:20 UTC, az 110°, alt 8°, 75° FoV) framed on
  the anti-solar horizon to expose the Belt of Venus arch and the
  Earth-shadow band underneath. The preset round-trips through the
  schema-versioned JSON session path like every other preset.

Validation:

- `astronomy::atmosphere` unit tests pin: at sun_alt = -2° the
  anti-solar arch fit gives `R > G > B` with `R > 1.05` at view_alt =
  5°; the Earth-shadow band has the lowest V-band luminance in the
  anti-solar half-sky at view_alt = 0° (dimmer than 2.5°, 5°, 8°,
  12°, 20°, 45°, 80°); both fits collapse to `[1, 1, 1]` for
  daylight, nautical and astronomical twilight; the combined Belt of
  Venus R/G ratio at the ROI A altitude (8°, anti-solar, depression
  3°) lies in `[1.15, 1.35]` while the Earth-shadow ROI B (0°,
  anti-solar) lies in `[0.85, 1.00]`, and ROI A > ROI B.
- Pinned validation scene: `docs/presets/sessions/civil-twilight-antisolar-tokyo.json`
  and `docs/assets/validation/civil-twilight-antisolar-tokyo.png` are
  recorded in `data/manifest.toml`; `make manifest-check` re-hashes
  both.
- Renderer tests continue to pass; the WGSL change is localized to the
  twilight composition region of `skyglow.wgsl` to minimise conflicts
  with the parallel V-25 / V-26 / V-28 work that also touches the
  same shader.

Documented limitations:

- The fits reproduce the chromaticity envelope of Lee & Hernández-Andrés
  2003, not their full multi-wavelength radiance tables; the
  multiplier targets the V-27 visual feature, not photometric truth
  outside the cited envelope. Multiple-scattering inside the Earth
  shadow is captured only as a residual blue-grey tint, not as a
  height-resolved twilight radiative transfer.

Hosts wired: CLI, viewer, web (all consume the same `skyglow.wgsl`
twilight pass; the new preset is exposed through the shared
`scene_preset_infos()` table).

Primary implementation areas:

- `crates/astronomy/src/atmosphere.rs`
  (`antitwilight_arch_radiance`, `earth_shadow_band_radiance`,
  `BELT_OF_VENUS_DEPRESSION_RANGE_DEG`, four V-27 unit tests).
- `crates/renderer/src/shaders/skyglow.wgsl`
  (`antitwilight_arch_multiplier`, `earth_shadow_band_multiplier`,
  V-27 application inside `twilight_sky_radiance`).
- `crates/common/src/presets.rs`
  (`ScenePresetArg::CivilTwilightAntisolarTokyo`,
  `scene_from_preset` case, info entry).
- `docs/presets/sessions/civil-twilight-antisolar-tokyo.json`,
  `docs/assets/validation/civil-twilight-antisolar-tokyo.png`,
  `data/manifest.toml`, `docs/scene-presets.md`,
  `docs/validation-gallery.md`, `ARCHITECTURE.md`, `ROADMAP.md`.

## L-14 Public demo gallery

Shipped a curated, narrated public demo page at
[`docs/demo-gallery.md`](demo-gallery.md) (and the Japanese mirror
[`docs/demo-gallery.ja.md`](demo-gallery.ja.md)) as the project front-door
showcase. Twelve of the most visually-striking deterministic scene
presets are surfaced with a one-line scientific caption, a 480 × 270
thumbnail, and the exact `--preset <name>` reproduction command:

- `tokyo-tonight`, `sunset`, `civil-twilight-antisolar-tokyo`,
  `moonlit-night`, `dark-sky`, `dark-sky-bortle-1`, `tokyo-bortle-8`,
  `solar-eclipse`, `venus-transit`, `jupiter-shadow-transit`,
  `all-sky-mollweide`, `galactic-north`.

Delivered as a docs + scripts + manifest PR (no engine or renderer
changes):

- `scripts/render-demo-gallery.sh` mirrors the validation-gallery
  script structure (`--update` / `--check` modes) but renders only the
  curated subset.
- `make demo-gallery` and `make demo-gallery-check` Makefile targets.
- `docs/assets/demo-gallery/*.png` — the 12 curated PNGs, each tracked
  in `data/manifest.toml` under `kind = "generated"` and
  `preprocessing = "scripts/render-demo-gallery.sh"`. `make
  manifest-check` (part of `make ci`) re-hashes the committed bytes,
  so silent drift fails CI even without running the screenshot
  regression.
- `README.md` and `README.ja.md` carry a prominent "Demo gallery"
  section near the top with three thumbnails (solar eclipse, Belt of
  Venus, Galilean shadow on Jupiter) and a `make demo-gallery`
  callout.

Reproduction discipline. Every gallery entry pins to a committed
session JSON in `docs/presets/sessions/` (catalog + atmosphere +
ephemeris versions live in the session schema), so a future re-render
reproduces the same scientific state. The L-14 surface is the
project's single source of truth for "this is what `stars` looks like";
future renderer slices that visibly change a curated scene must
regenerate `docs/assets/demo-gallery/<scene>.png` and update its
manifest hash in the same PR.

## Python bindings (`L-21`)

First rung of the PyO3 binding plan. Adds a self-contained
`bindings/python/` crate (`stars-py`) that wraps the read-only
`astronomy` + `catalog` public surface and is callable from a Jupyter
notebook so reviewers can reproduce the renderer's apparent positions
and magnitudes without spawning the CLI.

What shipped:

- `bindings/python/Cargo.toml`: `cdylib + rlib` PyO3 0.22 crate with
  the `extension-module` feature gated behind a default-off flag so
  `cargo check` does not need to find a Python interpreter. `abi3-py39`
  is enabled so one wheel works on every supported interpreter.
- `bindings/python/src/lib.rs`: `#[pyclass]` wrappers for `Observer`,
  `ApparentSun`, `ApparentMoon`, `SunMoon`, `ApparentPlanet`,
  `ApparentGalileanMoon`, `ApparentTitan`, `StarCatalog`, and `Star`,
  plus module-level `apparent_sun_moon`, `apparent_planets`,
  `apparent_galilean_moons`, `apparent_titan`, and
  `observer_from_unix_seconds` entry points. Every apparent-body class
  carries an `.altaz(observer)` method that runs the same
  `equatorial_to_horizontal` helper the renderer uses, so notebook
  altitudes round-trip with the V-23 / V-24 / V-29 paths.
- `bindings/python/tests/smoke.py`: importable smoke test at the
  V-27 Tokyo civil-twilight epoch (2026-06-21T10:20:00Z), prints the
  Sun / Moon altitude and the first three planets plus the embedded
  catalog size. The script is what `maturin develop --features
  extension-module && python tests/smoke.py` verifies locally.
- `bindings/python/README.md`: build instructions, API surface table,
  scope and non-goals.
- `Makefile`: new `pyo3-check` target (`cargo check -p stars-py`)
  appended to `ci`. The wheel build is documented but **not** wired
  into CI — the GitHub Actions runner does not currently ship a
  Python toolchain.
- Workspace member: `bindings/python` added to root `Cargo.toml`.

Validation (4 unit tests in `bindings/python/src/lib.rs::tests`):

- `observer_round_trips_lat_lon_degrees` — lat/lon degree
  round-trip through the constructor matches inputs to < 1e-9.
- `apparent_planets_match_astronomy_order` — binding planet vector is
  byte-equivalent to `apparent_planets_topocentric` in both order and
  apparent RA.
- `embedded_catalog_loads_and_index_errors_safely` — catalog is
  non-empty, valid indices succeed, out-of-range index raises a
  `PyIndexError` (mapped to Python `IndexError`) rather than panicking.
- `moon_altitude_from_pure_rust_entry_point_is_finite` — the same
  pure-Rust entry point the Python `smoke.py` calls produces a finite
  Moon altitude within ±π/2 at the V-27 Tokyo epoch. This is the
  pinning gate the L-21 follow-up will tighten once a wheel matrix is
  in CI and the notebook side starts cross-checking.

Hosts wired: bindings live alongside the existing hosts; not a host
itself. CLI / viewer / web unchanged.

Docs updated: `ROADMAP.md` (`L-21` moved to `⏳ read-only surface
shipped`, follow-up scope tightened to wheel matrix + notebook port +
planning-helper expansion), `ARCHITECTURE.md` (new `bindings/python/`
row in the crate map), `bindings/python/README.md` (new).

Files touched:

- `bindings/python/Cargo.toml`, `bindings/python/src/lib.rs`,
  `bindings/python/tests/smoke.py`, `bindings/python/README.md`.
- `Cargo.toml` (workspace `members`).
- `Makefile` (`pyo3-check` target, `ci` append).
- `ROADMAP.md`, `ARCHITECTURE.md`, `PROGRESS.md`.

### `L-21` rung 2 — planning queries + session round-trip

Second rung of the PyO3 plan, expanding the surface from read-only
apparent-body lookups to the observation-planning and session layers a
reproducibility notebook actually drives.

What shipped (all in `bindings/python/`):

- Observation-planning bindings wrapping `astronomy::planning` 1:1:
  `evening_plan(observer)` → an `EveningPlan` pyclass carrying
  `.rows` (a `RiseTransitSet` per default planning body) and
  `.twilight` (an ordered `TwilightIndicator` band timeline);
  `rise_transit_set(observer, body, start_jd, end_jd)`,
  `twilight_indicators`, `twilight_band(sun_alt_rad)`, and
  `body_altitude_rad(observer, body)`. Bodies are named
  case-insensitively (`"sun"`, `"moon"`, `"mercury"` … `"neptune"`);
  unknown names raise a Python `ValueError`. Rise/transit/set times are
  `Optional[float]` UTC Julian Dates.
- Time helpers `julian_date_from_unix_seconds` and `jd_utc_to_unix_ms`
  so notebooks can move between POSIX time and the JD planning windows.
- A mutable `Session` pyclass that loads/saves the `crates/common`
  `StarSession` JSON schema (camelCase, `SESSION_SCHEMA_VERSION = 5`).
  Constructors: `Session(lat, lon, jd_utc, azimuth_deg=…,
  altitude_deg=…, fov_deg=…)`, `Session.from_observer(observer, …)`,
  `Session.from_json(text)`, `Session.load(path)`. Serialisers:
  `to_json(pretty=True)`, `save(path)`. Typed observer/time/view
  accessors (`latitude_deg`, `longitude_deg`, `jd_utc`, `jd_ut1`,
  `jd_tt`, `jd_tdb`, `azimuth_deg`, `altitude_deg`, `fov_deg`,
  `schema_version`); setting `jd_utc` recomputes the dependent scales
  via `TimeScales::from_utc_julian_date`. `.observer()` bridges a loaded
  session into the query API so apparent-body calls reproduce the
  renderer's numerics. Every field the binding does not edit is
  preserved verbatim on round-trip.
- Dependency discipline: the session round-trip rides on `serde_json`
  only — **not** the host `stars-host-common` crate (clap / chrono /
  wgpu) — keeping the binding off the GPU / CLI path. `Session`
  constructors seed from the committed `dark-sky` preset
  (`include_str!`) so the produced JSON always tracks the current
  schema without the binding re-declaring fields.

Validation: five new in-crate Rust unit tests (nine total) — evening
plan window contiguity + body-count, named-body validation, embedded
template schema guard (`schemaVersion == 5`), session edit/round-trip
with time-scale recomputation + preservation of an untouched field, and
`from_observer` time-scale preservation. `bindings/python/tests/smoke.py`
extended to print the evening plan, twilight timeline, and a session
round-trip → `Observer` → apparent-Sun altitude. `make ci` passes
(`cargo check -p stars-py`, `cargo clippy --all-targets -D warnings`,
`cargo test --workspace`).

Docs updated: `ROADMAP.md` (`L-21` status line + detailed item),
`bindings/python/README.md` (planning + session API tables), this entry.

Files touched: `bindings/python/Cargo.toml`,
`bindings/python/src/lib.rs`, `bindings/python/tests/smoke.py`,
`bindings/python/README.md`, `ROADMAP.md`, `PROGRESS.md`.

### `L-21` rung 3 — occultation / eclipse surface (item complete)

Final rung: the binding now covers the V-51 occultation / eclipse
planning surface, closing the last API gap, so `L-21` moves to ✅.

What shipped (all in `bindings/python/`):

- Six free functions wrapping `astronomy` 1:1: `active_occluders(observer)`
  → `list[Occluder]` (the per-frame occlusion geometry the renderer
  consumes); `find_lunar_occultation(observer, planet, start_jd, end_jd)`
  (planet target) and `find_lunar_star_occultation(observer, dir_eq,
  start_jd, end_jd)` (date-equatorial unit-vector star target);
  `find_solar_eclipse`; `find_planet_transit` (Mercury / Venus only);
  and `find_mutual_planetary_occultation`. Each returns `Optional[...]`
  and validates body names with a Python `ValueError` (never a panic); a
  `end <= start` window returns `None`.
- Six new `#[pyclass]` wrappers — `ContactTimes` (P1..P4 as
  `Optional[float]` + `.as_tuple()`), `LunarOccultation`, `SolarEclipse`
  (with `.is_central()`), `PlanetTransit`, `MutualPlanetaryOccultation`,
  and `Occluder` (`target` / `kind` / `obscuration` / `front_radius_rad`
  / `front_dir_eq`). `kind` is the stable kebab label from the astronomy
  classifier. Added `planet_from_name` / `occluder_target_name` helpers
  (the latter maps `OccluderTarget::Planet(i)` through `Planet::ALL[i]`).
- `glam` added (workspace-pinned) only to build the date-equatorial unit
  direction for `find_lunar_star_occultation`.
- `bindings/python/examples/reproduce_session.py` — a binding-native
  reproducibility cross-check that loads a committed scene preset,
  rebuilds its `Observer`, and prints the apparent / planning /
  occultation numbers in-process (no CLI shell-out). The stdlib-only
  `examples/notebooks/session_reproducibility.py` is left untouched so
  `make notebook-check` keeps working without a Python extension build.

Validation: two new in-crate Rust unit tests (eleven total) —
`occultation_finders_validate_and_map_options` (name validation + Option
mapping across all five finders, including the star path and the
identical-planet rejection) and `active_occluders_are_bounded_and_labelled`
(count ≤ `MAX_OCCLUDERS`, every target label resolvable, obscuration in
[0,1]). `smoke.py` extended to exercise the occultation / eclipse calls.
`make fmt` then `make ci` pass (`cargo check -p stars-py`,
`cargo clippy --all-targets -D warnings`, `cargo test --workspace`,
`make notebook-check`, frontend typecheck).

Honest residual (out of scope, ⬜): a `pip install stars-py` wheel matrix
built via maturin in CI for Linux / macOS / Windows. This is packaging
infrastructure that needs a Python toolchain provisioned in the GitHub
Actions job; the binding API, docs, and tests are complete and local
`maturin develop` is documented, so the row is ✅ with the wheel matrix
tracked as a separate CI-infra task.

Docs updated: `ROADMAP.md` (`L-21` row → ✅, block reworded with the
occultation surface + distribution follow-up), `ARCHITECTURE.md`
(`bindings/python` row), `bindings/python/README.md` (occultation / eclipse
API table + example), this entry.

Files touched: `bindings/python/Cargo.toml`,
`bindings/python/src/lib.rs`, `bindings/python/tests/smoke.py`,
`bindings/python/examples/reproduce_session.py`,
`bindings/python/README.md`, `ROADMAP.md`, `ARCHITECTURE.md`,
`PROGRESS.md`.

## Headless HTTP server host (`L-22`)

A new native host, `stars-server` (`apps/server/`), wraps the existing
render pipeline in an axum HTTP service. Sessions go in as JSON, PNGs
come out; built-in scene presets are reachable as endpoints. The host
reuses `stars-host-common` for **every** non-HTTP concern — session
loading and validation, preset resolution, render-options plumbing,
and the wgpu device + readback pipeline that previously lived inline
in `apps/cli/src/main.rs`. The render code now lives in
`crates/common/src/render.rs` and both the CLI and the server call it,
so the two hosts cannot drift on the GPU init path or the catalog
resolution rule.

Shipped capabilities:

- `GET /healthz` — 200 OK with `{ status, service, version }`.
- `GET /presets` — array of every built-in `ScenePresetInfo`, kebab-id
  keyed.
- `GET /presets/{id}` — that preset's effective
  schema-versioned `StarSession` JSON, exactly equivalent to the
  files under `docs/presets/sessions/`.
- `POST /render` (body: session JSON, query: `width`, `height`,
  `skyglow`) — returns PNG bytes with `Content-Type: image/png`.
  Resolution is clamped to `[16, 8192]`. The render runs on a
  `tokio::task::spawn_blocking` worker so the runtime stays
  responsive while wgpu owns a thread.
- JSON error envelope: `{ error, detail }` for 400 / 404 / 500.
- `Makefile` target `make server` runs the binary in release mode
  with `SERVER_ARGS` overridable (`make server SERVER_ARGS="--port
  9000"`).

Primary implementation areas:

- `apps/server/src/main.rs`: routes, axum wiring, render handler.
- `crates/common/src/render.rs`: `RenderOptions`,
  `render_scene_pixels`, `render_scene_from_catalog_path`, and
  `encode_png` — the host-tier render glue extracted out of the
  CLI.
- `apps/cli/src/main.rs`: now a thin caller of
  `render_scene_from_catalog_path` (≈130 lines of GPU init removed
  here, no behaviour change).
- `crates/common/Cargo.toml`: adds `wgpu`, `pollster`, `image` — the
  three dependencies that were already on every native host but
  duplicated per crate; they now live exactly once.

Validation:

- `apps/server/src/main.rs::tests::healthz_and_presets_round_trip`
  binds an ephemeral port, drives the real `axum::serve` loop, and
  asserts the JSON shape of `/healthz`, `/presets`, and the 404
  envelope on an unknown preset id. The `/render` route is
  intentionally *not* tested here — GPU adapter availability is
  not guaranteed on every CI lane and the existing `examples` and
  validation gallery already pin the GPU path.
- The catalog resolution rule (`scene.catalog.path` → server
  default) is shared with the CLI through
  `render_scene_from_catalog_path`, so any catalog-path regression
  shows up on both hosts.

Hosts wired: server (new), CLI (refactored to reuse shared render),
viewer / web unchanged.

### `V-56` Object search, GoTo, and info panel — shipped (web + CLI + viewer)

The loaded catalog data and apparent-position pipeline now has a search
surface in the web app. Typing into the top-of-canvas search box debounces
a lookup against an in-memory inverted index covering ~1.2k bright named
stars (HYG `proper` / Bayer / Flamsteed / HR / HD / HIP), the 110 Messier
objects, the bright NGC / IC subset, and the nine solar-system bodies
(Sun + Moon + planets minus Earth). Picking a result calls into the WASM
GoTo path, which evaluates the apparent topocentric direction at the
current observer + clock and slews the camera azimuth / altitude to match.
A non-modal info panel echoes RA / Dec (J2000), Alt / Az, magnitude where
available, observer-relative distance for solar-system bodies, and the
rise / transit / set times reused from `L-07`.

Ranking is intentionally simple and deterministic so the dropdown does
not reorder on minor query changes:

1. exact (case-insensitive) match on any identifier — score 0;
2. prefix match on a single identifier token — score 1;
3. case-insensitive substring match on the canonical name — score 2;
4. token-by-token substring match on Bayer / Flamsteed (`"alp cma"`
   matches `"Alp"` + `"CMa"`) — score 3.

Ties break on magnitude (brighter wins), then identifier ordering. The
top [`SEARCH_LIMIT_DEFAULT`](crates/catalog/src/search.rs) = 12 hits are
returned. Solar-system bodies carry both English and Japanese aliases
(`Saturn` / `土星` / `どせい`) so bilingual users do not have to switch
input mode mid-search.

Primary implementation areas:

- `crates/catalog/data/named_stars.tsv` (manifest id
  `named-stars-search-index`): committed search index data extracted from
  HYG v4.2 by `scripts/extract-named-stars.py` with `proper`,
  `bayer + constellation`, `flam + constellation`, `HR`, `HD`, `HIP`
  columns plus J2000 RA / Dec / magnitude / distance. The compact rendered
  catalog is anonymous on the embedded path; this narrower TSV is the
  only place where catalog names survive into the WASM bundle until
  `L-18` (identifier preservation) lands.
- `crates/catalog/src/search.rs`: inverted-index + ranker; public
  `search(query, limit) -> Vec<SearchMatch>` API; stable `SearchId`
  encoding (`star:<idx>`, `m:<n>`, `ngc:<n>`, `ic:<n>`, `ss:<name>`) so
  the host round-trips ids through clicks and (future) URL parameters.
- `apps/web/src/lib.rs`: `StarView::lookup_object(query, limit) -> String`
  and `StarView::goto_object(id) -> String`. GoTo resolves the id back to
  a topocentric apparent `(alt, az)` via the existing
  `apparent_sun_topocentric` / `apparent_moon_topocentric` /
  `apparent_planet_topocentric` paths so the info panel sees the same
  ephemeris the renderer does, and packages a planning row reused from
  `rise_transit_set`.
- `apps/web/frontend/src/components/SearchPanel.tsx`: debounced input,
  ranked dropdown, info panel; pure inline styles consistent with the
  rest of the web HUD.
- `apps/web/frontend/src/App.tsx`: pulls the WASM proxy via a new
  `onSearchReady` callback on `StarCanvas`, then applies `(az, alt)` to
  the existing camera state setters — reusing the renderer's `LocalView`
  clamps without duplicating them in JS.

Validation (16 new tests in `catalog::search::tests`):

- proper-name lookup for Sirius (case-insensitive, brightest-star
  acceptance criterion);
- Bayer / Flamsteed / HD / HIP identifier round-trip (Sirius via
  `"Alp CMa"` / `"HD 48915"`, Vega via `"HIP 91262"`);
- token-by-token Bayer lookup (`"alp cen"` → Alpha Centauri);
- Messier and NGC by identifier and by nickname
  (`"M31"` / `"Pleiades"` / `"NGC 869"` / `"North America"`);
- planet / Sun / Moon lookup, including `"土星"` (Saturn);
- ranking-tie test pins the brighter-wins policy via Rigel ahead of
  fainter Beta-Orionis-like rows;
- empty / whitespace queries return zero hits;
- `SearchId::encode` ⇄ `SearchId::parse` round-trips for every variant.

#### CLI + desktop viewer parity

The CLI and desktop viewer now reach feature parity with the web host by
sharing a host-neutral resolver rather than reimplementing search:

- `crates/common/src/goto.rs`: `resolve_goto_query(query, observer)` and
  `resolve_goto_id(id, observer)` map a `catalog::search` hit to a
  `GotoTarget` carrying the apparent RA/Dec, topocentric (alt, az),
  magnitude, distance, and a human-readable `info_summary()`.
  `GotoTarget::local_view(fov)` produces a renderer `LocalView` centred
  on the target. The solar-system magnitude / distance conventions are
  kept identical to `apps/web` (Sun = −26.74, Moon magnitude `None`,
  planets from the ephemeris) so all three hosts report the same numbers.
- `apps/cli/src/main.rs`: `--goto <name>` resolves a query, overrides the
  local alt-az view to centre the target (keeping the current FOV), and
  prints `GoTo <summary>` before rendering. It applies after the scene is
  built so it also works with `--session` / `--preset`; a non-Earth
  viewpoint logs a note because the local view is ignored there.
- `apps/viewer/src/main.rs`: a startup `--goto` flag mirrors the CLI, and
  an interactive prompt opens on `/` — typed characters build a query
  shown live in the window title bar (used as a renderer-free info
  panel), Enter resolves + slews the camera + shows the target's info
  summary, Esc cancels. This avoids any renderer changes (no host-facing
  arbitrary-text HUD exists) and keeps to the existing keybinding
  conventions.

Click / hover *picking* of an arbitrary rendered body stays a web-only
affordance; the CLI is non-interactive and the viewer reaches the same
objects through the search prompt, so the `renderer::picking` R32Uint
buffer and a `SelectedTarget` session field remain open as a web
refinement and are not required for native-host parity.

Validation for the parity work:

- `crates/common/src/goto.rs` (9 tests): `resolve_goto_query` ranks
  `"Vega"` / `"M31"` / `"Saturn"` / `"土星"` to the expected ids and
  kinds, the planet path carries an AU distance + ephemeris magnitude,
  empty / unknown queries error, `local_view` centres on the target,
  `info_summary` is structured, and RA/Dec formatting is pinned at its
  bounds.
- `apps/cli/src/main.rs` (4 tests): `--goto` flag parsing (none /
  single / multi-word designation) plus the end-to-end parse → resolve →
  centred-view path.

Hosts wired: web (search panel + GoTo + info panel) / CLI (`--goto`) /
viewer (interactive `/` search prompt + startup `--goto` + title-bar
info panel).

---

## `V-55` Artificial satellites (TLE / SGP4) — shipped (CLI + viewer + web)

The renderer now has a TLE-driven artificial-satellite layer propagated by
SGP4, with Earth-shadow visibility and amateur-grade apparent magnitudes —
so the ISS, a Starlink train member, bright LEO satellites, and the
geostationary belt can be drawn for any observer / instant.

1. **What changed.** A new `astronomy::satellites` module parses two-line
   element sets and propagates them with the vendored pure-Rust `sgp4`
   crate (the Vallado et al. 2006 / Spacetrack Report #3 reference model),
   yielding a geocentric **TEME** position. The topocentric reduction
   reuses the existing WGS84 `observer_equatorial_position_km` helper —
   the TEME position and the observer position share the one GMST-defined
   inertial frame, so `r_sat − r_obs` gives right ascension / declination,
   and local sidereal time gives altitude / azimuth and slant range. A
   conical umbra / penumbra Earth-shadow test (built from the apparent
   Sun direction and distance, sharing the umbra-cone idea with `V-36`)
   gives the sunlit fraction, and the McCants / QuickSat "standard
   magnitude" convention (intrinsic V at 1000 km and half phase, scaled by
   range and a Lambertian phase fraction) gives apparent magnitude. The
   renderer carries a `SatelliteLayer` on `Camera` and packs a per-frame
   satellite uniform block; `shaders/skyglow.wgsl` draws each sunlit,
   above-horizon satellite as a neutral reflected-sunlight point sprite,
   or a great-circle motion streak when the exposure field is positive.

2. **Why it counts as complete.** SGP4 propagation matches the
   AIAA 2006-6753 reference vector to sub-km; the shadow and magnitude
   models are pinned; the layer renders visibly (a dark-sky ISS pass) and
   is wired into every host with a session + manifest-pinned data trail.

3. **Where it lives.** `crates/astronomy/src/satellites.rs`;
   `crates/renderer/src/camera.rs` (`SatelliteLayer`, `satellite_uniforms`,
   uniform block) + `crates/renderer/src/shaders/skyglow.wgsl`
   (`satellite_radiance` / `satellite_streak_mask`);
   `crates/common/src/satellites.rs` (curated TLE embed) +
   `crates/common/data/satellites/curated_tle.txt`; session schema v6 in
   `crates/common/src/session.rs`; the `iss-pass` preset in
   `crates/common/src/presets.rs`; `apps/cli`, `apps/viewer`, and
   `apps/web` (`StarView.set_satellites` + frontend settings control).

4. **Tests / validation.** `astronomy::satellites` unit tests pin the SGP4
   position against the AIAA 2006-6753 catalog-88888 reference vector
   (sub-km), the conical umbra/penumbra classification, the
   standard-magnitude reference geometry, the TLE parser, and a finite
   ISS apparent state; the renderer pins the satellite uniform packing for
   a visible ISS pass; `common` pins the curated snapshot parse; and
   `make manifest-check` pins the TLE snapshot bytes.

5. **Hosts wired.** CLI (`--satellites`, `--satellite-exposure-seconds`),
   viewer (`L` toggle + same flags), web (`set_satellites` WASM binding +
   settings toggle / exposure with session + URL round-trip). Live TLE
   fetch is opt-in only; the default render path uses the curated,
   manifest-pinned snapshot so renders stay deterministic.

References (also pinned in ROADMAP `V-55`): Vallado, D. A. et al. 2006,
AIAA 2006-6753; Hoots, F. R. & Roehrich, R. L. 1980, Spacetrack Report #3;
CelesTrak (celestrak.org); McCants (mmccants.org).

---

## `V-49` Comet rendering — shipped (CLI + viewer + web)

Named comets now render from a curated osculating-element snapshot with a
coma plus anti-solar ion tail and a β = 0.6 dust syndyne, so Halley,
Hale-Bopp, and C/2023 A3 (Tsuchinshan-ATLAS) can be drawn for any observer /
instant.

1. **What changed.** A new `astronomy::comets` module stores comet orbits as
   J2000-ecliptic osculating Keplerian elements (Marsden / MPC convention)
   and propagates them two-body from the time of perihelion passage — the
   elliptical, parabolic (Barker), and hyperbolic conics are all handled. The
   heliocentric Gauss-vector state (position + velocity) is rotated into the
   J2000 equatorial frame, where it shares one frame with the Earth's VSOP87D
   position (de-precessed mean-of-date → J2000 by the inverse IAU 2006
   precession matrix); the geocentric vector is light-time corrected and
   reduced to RA/Dec, with WGS84 topocentric parallax for the renderer. Coma
   photometry uses the Bobrovnikoff-Bowell law
   `m1 = M1 + 5 log₁₀ Δ + K1 log₁₀ r`; the ion tail follows the prolonged
   anti-solar radius vector and the dust tail a representative
   Finson-Probstein β = 0.6 syndyne. The renderer carries a `CometLayer` on
   `Camera`, packs a per-frame comet uniform block, and
   `shaders/skyglow.wgsl` draws each comet as a soft 1/ρ coma plus
   great-circle ion / dust tail streaks in a self-contained `comet_radiance`
   pass.

2. **Why it counts as complete.** Two-body propagation is pinned (exact `r =
   q` at perihelion; Halley at the 1986-04-10 close approach matches the
   recorded apparition — RA ≈ 15.5 h, Dec ≈ −47°, Δ ≈ 0.42 AU — to
   arcminutes); the magnitude law and tail geometry are tested; the layer
   renders visibly (the `bright-comet` C/2023 A3 preset) and is wired into
   every host with a session + manifest-pinned data trail.

3. **Where it lives.** `crates/astronomy/src/comets.rs`;
   `crates/renderer/src/camera.rs` (`CometLayer`, `comet_uniforms`, appended
   `CameraUniform` comet block) + `crates/renderer/src/shaders/skyglow.wgsl`
   (`comet_radiance`); `crates/common/src/comets.rs` (curated element embed)
   + `crates/common/data/comets/elements.csv`; a backward-compatible
   `#[serde(default)]` `comets` session field (no schema-version bump) in
   `crates/common/src/session.rs`; the `bright-comet` preset in
   `crates/common/src/presets.rs`; `apps/cli` (`--comets`), `apps/viewer`
   (`--comets` + `C` key), and `apps/web` (`StarView.set_comets` + frontend
   settings toggle, localStorage, and shareable-URL state).

4. **Tests / validation.** `astronomy::comets` pins the perihelion-radius
   invariant, post-perihelion distance growth, the Halley 1986 close-approach
   position, the magnitude law, unit anti-solar tail directions, the
   parabolic / hyperbolic branches, and the CSV parser; `make manifest-check`
   pins the element snapshot and the `bright-comet` preset.

5. **Hosts wired.** CLI (`--comets`), viewer (`--comets` + runtime `C`
   toggle), web (`set_comets` WASM binding + settings toggle with session /
   URL round-trip). The layer is off by default so the dark-sky composition
   is unchanged; the default render path uses the curated, manifest-pinned
   snapshot so renders stay deterministic.

References (also pinned in ROADMAP `V-49`): Finson, M. L. & Probstein, R. F.
1968, ApJ 154, 327; Marsden, B. G. & Williams, G. V., MPC element format;
Bobrovnikoff, N. T. 1942, ApJ 95, 71; Bowell, E. et al. 1989. Orbital data
via NASA/JPL SBDB.

---

### L-19 SIMBAD / VizieR deep links

The info panel / GoTo metadata now carries canonical CDS lookup URLs built
from the identifiers an object already exposes — no new catalogue ingest
and no identifier-preservation rework (`L-17` / `L-18` stay open). Nothing
in this path makes a network call and the renderer is untouched, so
deterministic renders remain offline and reproducible.

Primary implementation areas:

- `crates/catalog/src/links.rs`: `StarIdentifiers` (HIP / HD / HR /
  proper name / catalogue designation + J2000 RA / Dec) plus the pure
  builders `simbad_query_url(&StarIdentifiers) -> String` and
  `vizier_query_url(&StarIdentifiers) -> String`. SIMBAD prefers an
  identifier query (`sim-id?Ident=…`) in catalogue priority order
  HIP → HD → HR → proper name → designation, and falls back to a J2000
  cone search (`sim-coo?Coord=…&Radius=2&Radius.unit=arcmin`). VizieR
  always uses the positional cone search
  (`VizieR-4?-c=…&-c.rs=2`). A dependency-free percent-encoder keeps the
  URLs `href`-safe (space → `+`, RFC-3986-unreserved pass-through). The
  helper lives in `catalog` so the WASM web binding and the native hosts
  share one source of truth.
- `crates/common/src/lib.rs`: re-exports `simbad_query_url`,
  `vizier_query_url`, and `StarIdentifiers` on the documented
  `stars_host_common` path (the original `L-19` scope named
  `crates/common`; the implementation sits in `catalog` only so the
  WASM binding can reach it without pulling `clap` / `chrono`).
- `crates/common/src/goto.rs`: `GotoTarget` gains `simbad_url` /
  `vizier_url`, populated from the resolver's existing fields. Solar-
  system bodies expose `None` because the CDS stellar archives do not
  catalogue them.
- `apps/web/src/lib.rs`: `goto_object` JSON now emits `simbadUrl` /
  `vizierUrl` (null for solar-system bodies).
- `apps/web/frontend/src/components/SearchPanel.tsx`: the info panel
  renders the SIMBAD / VizieR links behind an opt-in checkbox persisted
  on its own `localStorage` key (`stars.externalLinks.v1`, default off),
  independent of the session schema. Links open in a new tab with
  `rel="noopener noreferrer"`.
- `apps/cli/src/main.rs` / `apps/viewer/src/main.rs`: echo the SIMBAD /
  VizieR URLs in the GoTo metadata output (CLI `println!`, viewer
  `log::info!`).

Validation:

- `catalog::links` (8 tests): identifier priority order, deep-sky
  designation encoding (`M 31`), VizieR positional cone search, signed /
  negative declination handling, RA wraparound, and the no-identifier
  coordinate fallback all match the CDS URL specification.
- `stars_host_common::goto` (3 tests): named star → HIP `sim-id` link,
  Messier → designation `sim-id` link, solar-system → no links.
- The web frontend ships no JS test harness, so link rendering is
  covered by the Rust JSON-emission tests plus the `tsc --noEmit`
  type-check in `make ci`; a `vitest` browser test is deferred with the
  rest of frontend test-infra.

Hosts wired: web (info panel links) / CLI (`--goto` metadata) / viewer
(GoTo metadata log).

---

## `V-50` Output colour management (sRGB / Display-P3 / Rec.2020) — shipped (CLI + viewer + web)

1. **What changed.** The renderer now selects an output colour space at the
   final tone-map step and tags the output with the chosen primaries, so the
   same scene reproduces the same colour on a calibrated wide-gamut screen.
   Previously the pipeline emitted untagged sRGB-encoded RGB.
2. **Why it counts as complete.** The gamut math is unit-tested, the transform
   is wired through the shared tonemap pass, the choice is persisted in the
   session JSON (schema v7), and all three hosts expose a control; a smoke
   render confirms sRGB PNGs carry an `sRGB` chunk while Display-P3 / Rec.2020
   PNGs carry a `cHRM` primaries chunk and differ in pixels from sRGB.
3. **Where it lives.** `crates/renderer/src/colourspace.rs`
   (`OutputColourSpace` + CSS-Color-4 linear gamut matrices + chromaticities),
   `crates/renderer/src/tonemap.rs` + `shaders/tonemap.wgsl` (binding-4
   `ColourManagementUniform`, applied after the Reinhard operator),
   `crates/common` (`OutputColourspaceArg`, `SessionScene.output_colourspace`,
   `outputColourspace` JSON field, schema bump to v7), `apps/cli`
   (`--output-colourspace`, `png`-crate encoder with `sRGB` / `cHRM` chunks),
   `apps/viewer` (`--output-colourspace`), and `apps/web`
   (`StarView.set_output_colourspace`, settings-panel dropdown, persistence).
4. **Tests / validation.** `renderer::colourspace` pins the sRGB identity, a
   primary's chromaticity preserved across each transform, the wide-gamut
   saturation check, and string round-trips; `stars-host-common` pins the
   `OutputColourspaceArg` ↔ engine round-trip and the v7 `outputColourspace`
   session field; the Python-binding template test tracks schema v7.
   Committed preset sessions and `data/manifest.toml` were regenerated.
5. **Hosts wired.** CLI, viewer, web. The host swap-chain / PNG keeps the sRGB
   transfer function (Display-P3 is exact; Rec.2020 uses the sRGB transfer as a
   documented approximation), and the web canvas falls back to sRGB where
   wide-gamut presentation is unsupported.

## `V-45` Telescope-side optical artifacts — shipped (CLI + viewer + web)

The eyepiece simulation (`V-43`) is no longer just a magnification / FOV
calculator: bright stars now show the diffraction artifacts an observer
actually sees through a given telescope — the Airy disc and rings, spider
diffraction spikes, the achromat colour fringe, and an exit-pupil vignette.

1. **What changed.** A new `OpticalDesign` enum (`Refractor { achromat,
   focal_ratio }`, `Newtonian { spider_vanes }`, `SchmidtCassegrain {
   obstruction_pct }`) and an OTA roll angle were added to
   `EyepieceSimulation`. The renderer derives the Airy radius (`1.22 λ/D`,
   Born & Wolf §8.5), the linear central-obstruction ratio, the spider-vane
   count, and the achromat secondary-spectrum fringe, packs them into two
   `instrument_optics` `CameraUniform` rows (appended at the end of the
   struct), and `shaders/star.wgsl` composites an instrument PSF on top of
   the Spencer eye PSF: an obstructed-aperture Airy pattern (`2J1(x)/x` via
   an Abramowitz & Stegun J1 approximation, annular for the obstruction),
   bidirectional spider spikes that rotate with the OTA (even vane counts
   give `n` arms, odd counts `2n`), a per-channel chromatic ring shift for
   achromats, and a cos⁴ field vignette. The instrument path is gated on
   eyepiece-active-in-perspective-Earth-view, so every other render stays
   bit-identical to the naked-eye pipeline.

2. **Why it counts as complete.** The Airy radius matches the Born & Wolf
   reference to <1%, the diffraction pattern resolves only at high power
   (the Airy pixel radius scales with magnification, as in a real
   eyepiece), and the model is wired into all three hosts behind the
   existing eyepiece toggle. A naga WGSL parse + validate test guards the
   shader and the Rust↔WGSL `CameraUniform` layout in CPU-only CI.

3. **Where it lives.** `crates/renderer/src/camera.rs` (`OpticalDesign`,
   `EyepieceSimulation` extensions, `instrument_optics_uniforms`,
   `CameraUniform` rows) + `crates/renderer/src/shaders/star.wgsl`
   (Bessel J1, Airy, spikes, vignette composite); `crates/common`
   (`EyepieceOverrides` design fields, `eyepiece_from_args`,
   `OpticalDesign` re-export); `apps/cli` (`--telescope-design`,
   `--spider-vanes`, `--ota-rotation-deg`); `apps/viewer` (`O` / `[` / `]`
   keybinds + the same flags); `apps/web` (`set_telescope_optics` WASM
   binding, settings-panel design / vane / rotation controls, URL params,
   localStorage).

4. **Tests / validation.** `renderer::camera` unit tests pin the Born &
   Wolf Airy radius, the per-design obstruction / vane mapping, the
   achromat chromatic fraction, the eyepiece-mode gate, and the
   magnification→Airy-pixel scaling; `shaders_parse_and_validate` parses
   and validates the WGSL with naga.

5. **Hosts wired.** CLI, viewer, web (when eyepiece mode is active). The
   optical design is a live render control this cycle; persisting it in the
   shared JSON session schema is a documented follow-up (it can append
   optional fields without a schema bump).

References (also pinned in ROADMAP `V-45`): Born & Wolf 1999 §8.5;
Conrady 1929; Suiter 2008; Rutten & van Venrooij 1988.

## `V-47` Meteor shower display — shipped (CLI + viewer + web)

The renderer now draws annual meteor showers as deterministic, radiant-based
streaks, so an observer at a shower's peak sees Perseids / Geminids / etc.
radiating from the right point at a physically-grounded rate.

1. **What changed.** A new `astronomy::meteors` module ships the IMO Working
   List of Visual Meteor Showers as a transcribed-constant catalog (radiant
   α/δ, peak solar longitude, ZHR, population index `r`, velocity, activity
   slope `B`). `zhr_at_solar_longitude` applies the Jenniskens 1994
   double-exponential activity profile so the rate is date-dependent;
   `observed_rate_per_hour` inverts the Koschack & Rendtel 1990 ZHR reduction
   to the rate a real sky shows (`n = ZHR·sin h_R·r^(lm−6.5)/F`);
   `meteor_stream` draws a deterministic SplitMix64-seeded Poisson sample of
   shower + sporadic meteors, time-binned on `(seed, jd_utc/window)` so the
   same session reproduces the same stream on every host. The renderer carries
   a `MeteorLayer` on `Camera`, packs each streak (head + tail unit vectors,
   peak magnitude) into a `meteor_segments` uniform block appended at the END
   of `CameraUniform`, and `shaders/skyglow.wgsl::meteor_radiance` draws each
   above-horizon streak via the shared great-circle mask from a single
   composition insertion point (kept isolated from the parallel V-48 / V-49
   shader work). One-frame appearance, no persistent train.

2. **Why it counts as complete.** The observed-rate formula is pinned to the
   Koschack-Rendtel reference value; the activity profile and solar-longitude
   clock are pinned; the stream is deterministic and host-independent; the
   layer renders visibly and is wired into every host with an optional,
   schema-stable session field.

3. **Where it lives.** `crates/astronomy/src/meteors.rs`;
   `crates/renderer/src/camera.rs` (`MeteorLayer`, `meteor_uniforms`,
   `MAX_METEORS`, uniform rows) + `crates/renderer/src/shaders/skyglow.wgsl`
   (`meteor_radiance`); `crates/common/src/session.rs` (`SessionMeteors`,
   appended with `skip_serializing_if` so presets stay byte-identical and the
   schema version is unchanged) + `crates/common/src/render.rs`; `apps/cli`,
   `apps/viewer` (`M`-key toggle), and `apps/web` (`StarView.set_meteors` +
   settings controls).

4. **Tests / validation.** `astronomy::meteors` pins the Koschack-Rendtel
   observed rate (≈ 58.4 m/h for Perseids at h_R = 60°, lm 6.0), the
   standard-conditions ZHR recovery, the Jenniskens activity profile, the
   Perseid-maximum solar longitude, and stream determinism / seed-sensitivity;
   the renderer pins meteor-uniform packing (unit-length streaks, the
   `MAX_METEORS` cap, external-viewpoint suppression); `common` round-trips an
   enabled meteor layer through the session schema.

5. **Hosts wired.** CLI (`--meteors`, `--meteor-seed`, `--meteor-rate-scale`,
   `--meteor-window-seconds`), viewer (same flags + `M` runtime toggle), web
   (`set_meteors` WASM binding + settings panel toggle / seed / rate-scale
   with localStorage + session round-trip).

References (also pinned in ROADMAP `V-47`): Koschack, R. & Rendtel, J. 1990,
WGN 18, 44; Jenniskens, P. 1994, A&A 287, 990; IMO Meteor Shower Calendar;
McKinley, D. W. R. 1961, *Meteor Science and Engineering*.

## `V-48` Aurora display — shipped (CLI + viewer + web)

1. **What changed.** The renderer can paint the statistically-expected
   auroral oval for a supplied planetary Kp index: a green O I 557.7 nm
   discrete arc with a higher-altitude red O I 630.0 nm band and a magenta
   N₂ lower border, positioned for the observer's geomagnetic latitude.
   Off by default, so the dark-sky composition is unchanged.
2. **Why it counts as complete.** The oval-boundary, geomagnetic-coordinate,
   and apparent-altitude models are pinned (equatorward boundary ≈ 63° at
   Kp = 4, per Feldstein & Starkov 1967); the renderer uniform packing is
   tested; the layer renders visibly and is wired into all three hosts with
   an additive (no-schema-bump) session round-trip.
3. **Where it lives.** `crates/astronomy/src/aurora.rs`
   (`auroral_oval_boundary`, `geomagnetic_latitude_deg`,
   `bearing_to_geomagnetic_pole_rad`, `emission_apparent_altitude_rad`,
   `aurora_intensity`, `aurora_view`); `crates/renderer/src/camera.rs`
   (`AuroraLayer`, `Camera::aurora_uniforms`, `aurora_geometry` /
   `aurora_params` uniform rows appended at the end of `CameraUniform`);
   `crates/renderer/src/shaders/skyglow.wgsl` (self-contained
   `aurora_radiance`, Chamberlain 1961 emission-height structure);
   `crates/common` (`AuroraSeasonArg`, `aurora_from_args`,
   `SessionScene.aurora` / `SessionAurora` with `#[serde(default)]`);
   `apps/cli` (`--aurora` / `--aurora-kp` / `--aurora-season`), `apps/viewer`
   (`A`-key toggle + the same flags), and `apps/web`
   (`StarView.set_aurora`, settings-panel control, session / URL /
   localStorage round-trip).
4. **Tests / validation.** `astronomy::aurora` pins the Kp = 4 → 63°
   boundary, equatorward expansion with Kp, the Tromsø geomagnetic
   latitude, overhead/below-horizon emission geometry, the red-above-green
   curtain ordering, intensity monotonicity, a visible sub-auroral arc, and
   no aurora at a quiet low-latitude site; `renderer::camera` pins the
   default-off / external-viewpoint-off / visible-arc uniform packing;
   `stars_host_common::session` pins the additive `aurora` JSON round-trip
   and its disabled default when the key is absent.
5. **Model limit.** Corrected geomagnetic latitude uses a centered dipole
   (IGRF-13 2020 pole), not full AACGM, and the display is the statistically
   expected oval — no real-time curtains/rays/substorm motion. Documented in
   `VALIDATION.md`.
6. **Hosts wired.** CLI, viewer, web.

## `L-06` DE440 SPK Chebyshev reader — partial (reader shipped; kernel ingest deferred)

First slice of the DE440 ephemeris upgrade: the Chebyshev **kernel reader**
that DE440 ingest is built on.

1. **What changed.** New `crates/astronomy/src/spk.rs` (`astronomy::SpkKernel`):
   a dependency-free, endianness-aware NAIF **DAF/SPK** parser that reads the
   file record, summary/segment descriptors, and evaluates **Type 2**
   (Chebyshev position) and **Type 3** (Chebyshev position + velocity)
   segments. It resolves a body's state to the solar-system barycenter by
   walking the SPK center tree (e.g. Moon → Earth-Moon barycenter → SSB) and
   differencing, exposing `state_km` / `position_km` / `geocentric_equatorial`
   plus NAIF body-id constants. This reads real JPL `de440.bsp` / `de440s.bsp`
   kernels.
2. **Why it is partial (◑, not ✅).** No DE440 kernel is committed (32–110 MB
   binary) and the reader does not yet drive `apparent_sun/moon/planet`, so the
   rendered Sun/Moon/planet output is still the analytic VSOP87 / ELP2000
   visual tier (also the WASM fallback). Wiring a fetched kernel through the
   apparent-place pipeline and the JPL Horizons sub-arcsecond cross-check are
   deferred; `DATA_SOURCES.md` + `scripts/fetch-de440-subset.sh` document the
   kernel retrieval.
3. **Where it lives.** `crates/astronomy/src/spk.rs`, exported from
   `crates/astronomy/src/lib.rs` (`SpkKernel`, `SpkError`, `naif`).
4. **Tests / validation.** 8 unit tests against a synthetic, spec-accurate
   in-memory DAF/SPK image: Type 2 position vs closed-form Chebyshev, Type 2
   velocity vs finite difference, Type 3 explicit-velocity series, little- vs
   big-endian byte-identical decode, direct and barycenter-chained geocentric
   states, non-DAF / truncated rejection, and missing-coverage errors. No new
   committed data → `make manifest-check` unchanged.
5. **Hosts wired.** None yet (library-only; see deferral above).

References (pinned in ROADMAP `L-06`): Park, R. S. et al. 2021, AJ 161, 105
(DE440); Acton, C. H. 1996, Planet. Space Sci. 44, 65 (SPICE); NAIF DAF / SPK
Required Reading.

## Documentation progress

The documentation has been split into purpose-specific files:

- `README.md` / `README.ja.md` for entry points;
- `ROADMAP.md` for forward plan;
- `PROGRESS.md` for implementation log;
- `ARCHITECTURE.md` for crate boundaries and host integration;
- `CONTRIBUTING.md` for development process;
- `VALIDATION.md` for scientific validation policy;
- `DATA_SOURCES.md` for data provenance;
- `CITATION.cff`, `.zenodo.json`, and `docs/citation.md` for citation and
  release-archive metadata;
- `docs/standards-compliance.md` for IAU/SOFA-aligned routines,
  approximations, and non-goals.

## Next implementation log entries

When new work lands, add a short entry here with:

1. what changed;
2. why it counts as complete;
3. where the implementation lives;
4. what tests or validation pin the behaviour;
5. which hosts are wired, if applicable.
