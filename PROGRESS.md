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
  telescope eyepiece simulation (`V-43`), and the deep-sky overlay with
  Messier objects plus the bright NGC / IC subset (`V-42`).
- **Library track** — IAU-grade time / precession / nutation / aberration /
  proper motion (`L-01`–`L-05`), planning helpers (`L-07`, `L-08`),
  schema-versioned JSON sessions (`L-10`, `L-11`), deterministic scene
  presets (`L-12`), notebook reproducibility examples (`L-13`), catalog
  backend scaling scaffold (`L-16`), validation / demo gallery (`L-27`),
  citation metadata (`L-25`), standards-compliance document (`L-26`), and
  the data provenance manifest (`L-15`).

Still open:

- **Visual track** — dark-sky realism gaps (`V-25`, `V-27`, `V-28`;
  `V-26` lunar earthshine has shipped), site-specific
  brightness (`V-39`), niche visual features (`V-45`–`V-50`), rare
  phenomena (`V-47`–`V-49`). A follow-up PR will add a runtime streaming
  backend for the full ~14,000-entry OpenNGC catalogue on top of the
  embedded `V-42` subset shipped here.
- **Library track** — DE440 ephemerides (`L-06`), large catalog ingest
  (`L-17`), identifier preservation (`L-18`), SIMBAD / VizieR links
  (`L-19`), variable-star light curves (`L-20`), Python bindings (`L-21`),
  headless server (`L-22`), guided education (`L-23`), accessibility
  (`L-24`), public demo gallery (`L-14`), observation-planning polish
  (`L-09`).

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
