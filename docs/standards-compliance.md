# Standards compliance and model limits

`stars` uses standards and standards-adjacent references where they fit the
current renderer scope, but it does not claim to be a complete SOFA or JPL
Horizons replacement. This page separates implemented standards, intentional
approximations, and deliberate non-goals.

## Policy

- SOFA names below are reference algorithms or constants, not linked code.
- Public behaviour must state whether it is standards-grade, renderer-grade, or
  visual-only.
- Numerical changes to the implemented rows need pinned tests or a written
  reason why a test is not practical.

## Implemented standards or standard constants

| Area | Implemented in `stars` | Reference / SOFA analogue | Code |
|---|---|---|---|
| J2000.0 epoch | `JD 2451545.0` as the catalog / transform epoch | IAU-standard J2000.0 epoch | `crates/astronomy/src/time.rs` |
| UTC / TAI / TT separation | Built-in post-1972 leap-second table, `TT-TAI = 32.184 s` | IERS / USNO leap-second history; IAU 1976 TT definition | `crates/astronomy/src/time.rs` |
| UT1 | Explicit `dut1Seconds` path, defaulting to `UT1≈UTC` when unknown | IERS Earth-rotation convention | `crates/astronomy/src/time.rs`, sessions |
| Sidereal time | GMST polynomial for Earth rotation | IAU 1982 GMST form (`iauGmst82` analogue) | `crates/astronomy/src/time.rs` |
| IAU 2006 precession | P03 Fukushima-Williams angles and matrix | SOFA `iauPfw06` + `iauFw2m` path | `crates/astronomy/src/corrections.rs` |
| Mean obliquity | IAU 2006 mean-obliquity polynomial | SOFA `iauObl06` analogue | `crates/astronomy/src/corrections.rs` |
| Galactic coordinates | IAU galactic frame rotation from J2000/ICRS-like vectors | SOFA `iauIcrs2g` constants | `crates/astronomy/src/skyglow.rs` |
| Astronomical unit | Exact `149597870.7 km` | IAU 2012 Resolution B2 | `crates/astronomy/src/ephemeris.rs` |
| Nominal solar radius | `695700 km` | IAU 2015 Resolution B3 | `crates/astronomy/src/ephemeris.rs` |
| Observer ellipsoid | WGS84 equatorial radius and flattening for topocentric parallax | NGA/EPSG WGS84, not an IAU resolution | `crates/astronomy/src/ephemeris.rs` |
| Horizontal coordinates | ENU frame, azimuth north-through-east, altitude from horizon | Standard spherical astronomy convention | `crates/astronomy/src/horizontal.rs` |
| Constellation boundaries | IAU/Delporte boundary vertices rendered as overlays | IAU constellation regions / Delporte 1930 via CDS VI/49 | `crates/renderer/data/constellation_boundaries.csv` |

## Implemented approximations

| Area | Approximation | Why / limit | Code |
|---|---|---|---|
| TDB | Two-term `TDB-TT` expression with amplitude below about 2 ms | Adequate for current visual VSOP87/ELP inputs; not a relativistic time ephemeris | `crates/astronomy/src/time.rs` |
| Nutation | Four dominant luni-solar terms with IAU-2000-style signs | Targets the roadmap's renderer-scale ~9 arcsec budget; not full IAU 2000A/2000B | `crates/astronomy/src/corrections.rs` |
| Apparent sidereal time | Equation of equinoxes from compact nutation | Coupled to the compact nutation model | `crates/astronomy/src/corrections.rs` |
| Annual aberration | First-order aberration from approximate Earth orbital velocity | Captures the ~20 arcsec scale; omits full relativistic terms | `crates/astronomy/src/corrections.rs` |
| Refraction | Saemundsson/Meeus apparent-altitude correction with pressure / temperature scaling | Visual correction near the horizon; not a ray-traced atmosphere | `crates/astronomy/src/corrections.rs` |
| Sun | VSOP87/FK5 geocentric Sun plus WGS84 topocentric parallax | Visual / planning quality; `L-06` tracks DE440-class ephemerides | `crates/astronomy/src/ephemeris.rs` |
| Moon | Principal ELP2000-style lunar series plus WGS84 topocentric parallax | Visual Moon placement, phase, and moonlit sky; not final eclipse prediction | `crates/astronomy/src/ephemeris.rs` |
| Planets | VSOP87D apparent ecliptic coordinates with one light-time iteration and approximate magnitudes | Useful for naked-eye rendering; not publication-grade astrometry | `crates/astronomy/src/ephemeris.rs` |
| Earth shadow on Moon | Smooth geometric umbra aid | Visual eclipse aid only; not a contact-timing model | `crates/astronomy/src/ephemeris.rs` |
| Eclipse / occultation geometry (`V-51a`–`d`) | Pair-wise `ApparentDisk` classifier, lens-area obscuration, P1–P4 contact-time bisection, analytic-mask renderer path covering Moon → Sun (`V-51c`) and Moon → stars / planets (`V-51d`) | Visual eclipse / occultation rendering and detection; sub-second IOTA / NASA-canon contact accuracy is gated on the `L-06` DE440 upgrade. Transits across the Sun (`V-51e`) and mutual planetary occultation (`V-51f`) are not yet wired. | `crates/astronomy/src/occultation.rs`, `crates/astronomy/src/planning.rs`, `crates/renderer/src/shaders/{skyglow,star}.wgsl` |
| Airmass / extinction | Kasten-Young airmass and per-channel Hardie / Schaefer coefficients | Clear-site visual extinction, not site-calibrated spectroscopy | `crates/astronomy/src/photometry.rs` |
| Daylight sky | Hošek-Wilkie 2012 analytic full-spectral sky-dome (RGB path) | Tristimulus integration of the spectral model; ground albedo ∈ [0, 1], turbidity ∈ [1, 10], sun elev ∈ [0, π/2] (inputs clamped at the bounds). Solar disk drawn separately by the existing illuminant path. | `crates/astronomy/src/atmosphere/hosek_wilkie.rs`, `crates/renderer/src/shaders/skyglow.wgsl` |
| Twilight | Solar-depression radiance curve calibrated to clear-site V-band behaviour | Continuous visual model across civil / nautical / astronomical twilight | `crates/astronomy/src/skyglow.rs`, `crates/astronomy/src/atmosphere.rs` |
| Moonlight | Krisciunas-Schaefer phase law with approximate moonlight colour | Rendering illuminance, not calibrated lunar photometry | `crates/astronomy/src/illuminants.rs` |
| Diffuse night sky | Analytic Leinert-inspired ISL/DGL/zodiacal/airglow/dust model | Visual dark-sky Milky Way and background; not radiometric survey data | `crates/astronomy/src/skyglow.rs` |
| Star colour | B-V to temperature to blackbody/CIE/sRGB approximation | Catalogue-colour visualization from limited HYG inputs | `crates/catalog/src/color.rs` |
| Human vision | CIE/Ferwerda/Reinhard/Pattanaik-inspired mesopic, scotopic, and tonemap helpers | Display-facing perception model, not metrology | `crates/astronomy/src/photometry.rs`, renderer shaders |
| Artificial satellites (`V-55`) | SGP4 (Vallado 2006 / Spacetrack #3, via the `sgp4` crate) on a curated manifest-pinned TLE snapshot; TEME treated as the J2000-ish equatorial frame; conical umbra/penumbra Earth-shadow visibility; McCants/QuickSat standard-magnitude photometry | Naked-eye satellite rendering and visibility, validated against the AIAA 2006-6753 reference vector (sub-km). TLEs are epoch-local (drift over weeks); not operational tracking, and apparent magnitudes use a hand-curated intrinsic-magnitude table, not a cross-section/BRDF model. | `crates/astronomy/src/satellites.rs`, `crates/renderer/src/shaders/skyglow.wgsl` |

## SOFA routines deliberately not implemented as complete routines

| SOFA routine family | Status in `stars` | Reason |
|---|---|---|
| `iauAtci13`, `iauAtco13`, `iauApci13` apparent/observed-place pipelines | Not implemented end-to-end | The renderer applies selected corrections separately so hosts can expose and document each approximation. |
| `iauNut00a`, `iauNut00b`, `iauPn06a` full nutation / precession-nutation pipelines | Replaced by compact nutation plus IAU 2006 precession | Keeps the WASM renderer small while meeting current visual tolerance; full tables are a future precision upgrade. |
| `iauEra00`, `iauGst06a`, `iauSp00`, `iauPom00`, `iauC2t06a` Earth rotation / CIO / polar-motion path | Not implemented | Current sessions expose UT1/DUT1 but do not fetch polar motion or use a CIO-based terrestrial-to-celestial matrix. |
| `iauTttdb`, `iauTdbtt`, TCB/TCG helpers | Replaced by a two-term TT→TDB approximation | Current ephemerides are visual approximations; relativistic time ephemerides belong with publication-grade ephemeris work. |
| `iauEpv00`, `iauPlan94`, SOFA solar-system helpers | Not used as the ephemeris source | Current code uses `astro` VSOP87/ELP-style series; `L-06` tracks DE440-class replacement. |
| `iauRefco` / SOFA observed-place refraction constants | Not used directly | Refraction uses Saemundsson/Meeus apparent-altitude correction with explicit pressure/temperature controls. |

## Deliberate non-goals today

- No bundled SOFA source code and no promise of bit-for-bit SOFA parity outside
  the constants and formula families named above.
- No full IAU 2000A/2000B nutation table, CIO locator, Earth Rotation Angle
  pipeline, polar motion, ocean/solid-Earth tides, or automatic IERS Bulletin A
  download.
- No pre-1972 UTC frequency-steering history; dates before the leap-second era
  clamp to the first table entry rather than modelling historical UT2/UTC.
- No TCG/TCB, relativistic light deflection, stellar radial-velocity
  perspective acceleration, or full apparent-place reduction for arbitrary
  catalog stars.
- No DE440 / SPICE kernel reader yet; current Sun/Moon/planet states are visual
  approximations and should not be described as publication-grade ephemerides.
- No terrain horizon, clouds, weather, local light pollution, or colour-managed
  display calibration.
- No constellation point-in-region classifier; boundaries are rendered for
  education, not used as authoritative catalog labels.
- **No live TLE fetch in the default render path (`V-55`).** The artificial-
  satellite layer ships a curated, manifest-pinned CelesTrak snapshot
  (`crates/common/data/satellites/curated_tle.txt`, manifest id
  `celestrak-tle-curated-2026-05`) embedded at build time, so default renders
  are deterministic and reproducible offline. Fetching fresh TLEs from
  CelesTrak / Space-Track at runtime is an opt-in host responsibility, not a
  default: it makes renders non-reproducible (positions depend on the
  download time), depends on network availability, and the elements are only
  accurate near their epoch. The shipped snapshot is for demonstration /
  validation of the SGP4 pipeline, not for operational conjunction or
  re-entry tracking. Regenerate the snapshot with
  `scripts/fetch-satellite-tle.sh` and refresh the manifest hash.

## How to cite standards-dependent output

For screenshots or tables, cite the `stars` release or commit plus the JSON
session. If the result depends on a row in the approximation table, name that
approximation in the caption or methods note. For external comparison, record
the comparison service/version, time scale, observer, target, coordinate frame,
and tolerance as described in [`VALIDATION.md`](../VALIDATION.md).
