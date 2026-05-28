# Validation

`stars` aims to be visually useful while keeping the scientific choices behind
its output explicit. This document records the validation policy: what should be
pinned by tests, what external references matter, and where the current limits
are. The companion [`docs/standards-compliance.md`](docs/standards-compliance.md)
page lists implemented IAU/SOFA-aligned routines, approximations, and non-goals.

The roadmap-level goal is not that every subsystem is publication-grade today.
The goal is that each approximation is named, tested, and easy to replace when
a higher-precision model lands.

## Validation principles

1. **Name the model.** If code implements a standard, paper, or approximation,
   the code or docs should say which one.
2. **Pin representative values.** Numerical behaviour should have tests that
   fail on silent drift.
3. **Separate visual approximations from astronomical claims.** A rendering aid
   can be approximate, but it must not be documented as high-precision science.
4. **Prefer tolerances with meaning.** Use angular tolerances for positions,
   magnitude / luminance tolerances for photometry, and explicit time tolerances
   for planning events.
5. **Document model domains.** A daylight model, twilight model, refraction
   approximation, or ephemeris approximation should state where it is expected
   to be valid.
6. **Make representative scenes reproducible.** Important visual claims should
   be tied to schema-versioned sessions or scene presets so screenshots can be
   regenerated instead of manually recreated.

## What must be tested

Add or update tests when changing any of these areas:

- leap-second handling;
- UTC / UT1 / TAI / TT / TDB conversion;
- sidereal time;
- precession, nutation, annual aberration, proper motion, refraction;
- equatorial / horizontal conversion;
- Sun, Moon, or planet apparent positions;
- topocentric parallax;
- moon phase or eclipse-darkening support;
- rise / transit / set;
- twilight band boundaries;
- magnitude, illuminance, luminance, extinction, skyglow, daylight, twilight,
  and tone reproduction reference models;
- catalog backend source labels, identifier preservation, filtering, coordinate
  conversion, and colour conversion;
- session schema migrations, scene preset parsing, and deterministic rendering
  inputs;
- notebook / CSV astronomy-table fixtures when example-facing Sun, Moon, or
  planet outputs intentionally change;
- visual-regression baselines when renderer changes are meant to preserve the
  appearance of representative scenes.

## Current validation coverage by subsystem

### Time systems

Current implementation:

- UTC civil input;
- UT1 for Earth rotation;
- TAI and TT from a built-in leap-second table;
- approximate TDB for ephemeris use;
- DUT1 defaults to zero unless supplied.

Validation expectation:

- J2000 and known leap-second boundaries should remain pinned.
- UT1-sensitive results should document whether DUT1 is supplied or assumed
  zero.

Current limitation:

- DUT1 is not automatically fetched from IERS bulletins.

### Coordinate transforms and stellar corrections

Current implementation includes:

- J2000 / ICRS-like Cartesian catalog positions;
- proper-motion propagation;
- IAU 2006 precession;
- compact IAU-2000-style nutation;
- first-order annual aberration;
- Saemundsson-style atmospheric refraction.

Validation expectation:

- Matrix transforms should preserve vector length within floating-point
  tolerance.
- Known epochs and simple geometric cases should be pinned.
- Refraction should be tested at representative altitudes and disabled / low
  pressure cases.

Current limitation:

- The nutation model is compact rather than a full SOFA-equivalent series.
- Refraction is an empirical visual correction, not a full ray-tracing model.

### Solar-system positions

Current implementation includes visually useful Sun, Moon, planet, Saturn
ring, Galilean-moon, and Titan apparent / topocentric inputs.

Validation expectation:

- Representative dates and observer locations should have pinned apparent
  directions.
- Topocentric correction should be tested separately from geocentric direction
  where possible.
- Angular radius and phase values should be pinned for simple dates.
- Galilean-moon angular separations should respect each moon's maximum
  apparent elongation from Jupiter.
- Titan's angular separation from Saturn should respect its maximum
  apparent elongation (≲ 3.4′ at closest opposition), and its apparent
  magnitude near a Saturn opposition should fall in the published
  Karkoschka 1998 band (V ≈ 8.3 ±0.4 near r ≈ 9 AU, Δ ≈ 8 AU).

Current limitation:

- `L-06` still tracks higher-precision DE440 / publication-grade ephemeris
  work. Do not describe the current stack as final research-grade ephemerides.
- The Galilean-moon backend ships at Meeus 1998 ch. 44 accuracy (V-52b):
  good for naked-eye / small-eyepiece identification near J2000. The
  pinned Horizons fixture (`data/horizons_galilean_moons.csv`,
  manifest id `horizons-galilean-moons-fixture`) at 1900 / 2000 /
  2100 epochs shows the in-plane RA-component offset error stays
  below ≈45″ across ±100 yr, while the out-of-plane Dec component
  rises to ≈180″ for Callisto at 2100 (the Meeus simplification
  drops the orbital inclination tilt, which dominates the weak
  axis). The full Lieske 1998 E5 precision upgrade tightens this
  budget to ~5″ across all four moons at every fixture epoch and is
  tracked as the dedicated rung `V-52b-E5` — that PR replaces the
  body of `lieske_e5::jovicentric_offset` and edits the single
  constant `MEEUS_GRADE_MAX_OFFSET_ERR_ARCSEC` in
  `moons::tests` from 200″ down to ≈5″.
- The Titan backend ships at Meeus 1998 ch. 45 accuracy (V-52c) — the
  same simplification of the TASS theory of Vienne & Duriez 1995 that
  the `astro` crate implements — with the same accuracy posture as the
  Meeus-grade Galilean backend. The full TASS1.7 precision upgrade
  with a Horizons-anchored ~5″ gate is tracked as the dedicated rung
  `V-52c-TASS17`.

### Eclipse / occultation geometry (`V-51`)

Current implementation:

- `crates/astronomy/src/occultation.rs` exposes the pair-wise
  `ApparentDisk`, `classify_disks`, `obscuration_fraction`, and
  `contact_times` helpers used by the solar-eclipse renderer
  (`V-51c`), the V-36 lunar-eclipse aid, and future lunar / planetary
  occultation and transit slices.
- `planning::solar_eclipse_state` and `planning::find_solar_eclipse`
  provide the per-frame state + window-search entry points the
  renderer and planning UI read.
- `planning::active_occluders` is the V-51b producer that builds the
  bounded analytic-mask list (`MAX_OCCLUDERS = 16`) consumed by the
  renderer's `CameraUniform::occluders` array. With V-51c + V-51d +
  V-51e + V-51f shipped it emits one Sun-targeted entry (Moon → Sun)
  when a solar eclipse is in contact, an always-on Stars-targeted
  entry (Moon → catalog stars; the star vertex shader culls sprites
  whose direction falls inside the front disk), one Planet-targeted
  entry per Moon ↔ planet pair currently in contact, a Sun-targeted
  entry per inner planet (Mercury, Venus) currently transiting the
  solar disk, and one Planet-targeted entry per planet ↔ planet pair
  currently in contact (V-51f assigns the closer planet as the front
  disk and the farther as the back; the same `OccluderTarget::Planet`
  variant the renderer already consumes for Moon-on-Planet).
- `planning::find_lunar_occultation(observer, body, start, end)` is
  the planning-side entry point for `V-51d` lunar occultations of
  stars and planets. `body` is
  `LunarOccultedBody::{Star { dir_date_eq }, Planet(p)}`; the helper
  drives a 1-minute scan to locate the closest approach and refines
  P1–P4 via the shared `contact_times` bisection.
- `planning::find_planet_transit(observer, planet, start, end)` is
  the planning-side entry point for `V-51e` Mercury / Venus transits
  across the Sun. The helper rejects outer planets up front, gates
  the peak scan on `planet.distance_au < sun.distance_au` (so the
  pure-geometry classifier never confuses a superior-conjunction
  near-alignment with a true transit), and refines P1–P4 via the
  shared `contact_times` bisection.
- `planning::find_mutual_planetary_occultation(observer, planet_a,
  planet_b, start, end)` is the planning-side entry point for `V-51f`
  mutual planetary occultation. The helper rejects same-planet pairs
  up front, drives a 1-minute scan for the peak (assigning the closer
  planet at peak as the front disk and the farther as the back), and
  refines P1–P4 via the shared `contact_times` bisection. The
  resulting `MutualPlanetaryOccultationEvent` carries the
  `front`/`back` planet identities, `kind`, `min_separation_rad`,
  `peak_obscuration`, `peak_jd_utc`, and `contacts`.

Validation expectation:

- Closed-form geometry must stay pinned for disjoint, touching,
  concentric annular, concentric total, point-source, and contact-time
  cases.
- Real historical eclipses must keep producing the correct kind and a
  plausible peak obscuration / totality duration:
  - 2024-04-08 Mazatl\u00e1n: `Total`, peak obscuration > 0.999,
    totality duration in `[60, 600]` s
    (`planning::tests::find_solar_eclipse_finds_2024_mazatlan_totality`).
  - 2012-05-21 Tokyo: `Annular` (or deep Partial) with peak
    obscuration > 0.80 and P1 \u2264 peak \u2264 P4
    (`planning::tests::find_solar_eclipse_finds_2012_tokyo_annular`).
  - Non-eclipse dates must return `None`
    (`planning::tests::find_solar_eclipse_returns_none_on_non_eclipse_day`).
- Real historical transits must keep producing the correct kind and a
  plausible peak obscuration / total duration:
  - 2012-06-06 Venus transit from Tokyo: `AnnularOrTransit` interior
    phase (P2/P3 present), peak obscuration in the area-ratio band
    `[5e-4, 2e-3]` (Venus apparent diameter \u224858\u2033, Sun \u22481890\u2033), total
    duration P1\u2192P4 in `[5, 8]` h
    (`planning::tests::find_planet_transit_finds_2012_venus_transit`).
  - Non-transit dates must return `None` for both Mercury and Venus
    (`planning::tests::find_planet_transit_returns_none_off_transit_day`).
  - Outer planets must be rejected up front
    (`planning::tests::find_planet_transit_rejects_outer_planets`).
  - Superior-conjunction near-alignments must not emit a Sun-targeted
    occluder (`planning::tests::active_occluders_skip_planet_on_sun_at_superior_conjunction`).

Current limitation:

- Sub-30-second P1\u2013P4 accuracy against NASA TP-2006-214141 stays
  gated on the DE440 upgrade tracked as `L-06`; the current VSOP87 /
  ELP2000 stack reproduces classification and peak obscuration but
  contact times agree only to within a few minutes.
- V-51 currently ships the Moon-on-Sun pair (`V-51c`), lunar
  occultation of catalog stars + the seven rendered planets
  (`V-51d`), Mercury / Venus transits across the Sun (`V-51e`), and
  mutual planetary occultation (`V-51f`). The V-51f slice reuses the
  V-51a/b primitives end-to-end; its producer contract is pinned by
  `planning::tests::active_occluders_emit_no_planet_on_planet_off_event`
  (no Planet-on-Planet entries on a normal day, discriminated from
  V-51d Moon-on-Planet entries by the front-disk radius) and the
  planning helper by
  `planning::tests::find_mutual_planetary_occultation_rejects_same_planet`
  and
  `planning::tests::find_mutual_planetary_occultation_returns_none_off_event`.
  Historical-event positive-detection validation against the next
  visible mutual occultation (2065-11-22 Venus occults Jupiter) is
  deferred until the DE440 ephemeris upgrade tracked as `L-06` lands;
  the current VSOP87 stack drifts a few minutes at that epoch, which
  is acceptable for the producer contract but not for sub-30 s P1–P4
  matching against the historical canon. The V-51b/d/f analytic-mask
  uniform path is pinned by
  `camera::tests::occluder_uniform_matches_moon_state_at_mazatlan_peak`
  (Sun-targeted entry in slot 0 + Stars-targeted entry in slot 1, both
  with direction equal to `moon_eq_illuminance.xyz` and radius equal
  to `moon_disk.x`; obscuration on the Sun entry equal to
  `solar_eclipse_state.y`),
  `camera::tests::occluder_uniform_zeros_on_external_or_atmosphere_off`,
  and `camera::tests::occluder_uniform_off_eclipse_emits_only_moon_on_stars`
  (exactly one entry, target code `-1`, lunar apparent radius). The
  star vertex shader cull is exercised by the producer-side test
  `planning::tests::active_occluders_off_eclipse_emits_only_moon_on_stars`
  and the planning-side
  `planning::tests::find_lunar_occultation_detects_synthetic_point_source`.
  Sub-second IOTA contact-time accuracy against published lunar
  occultation predictions remains gated on `L-06` (DE440 upgrade);
  the current VSOP87 / ELP2000 stack pins detection, classification,
  and contact-time bracketing to within minutes.

### Atmosphere and sky colour

Current implementation includes:

- stellar extinction;
- diffuse night-sky glow;
- zodiacal light, gegenschein, airglow, and dust extinction;
- solar and lunar illuminants;
- daylight scattering approximation;
- twilight radiance model;
- moonlit sky composition.

Validation expectation:

- Model-domain boundaries should be pinned: noon, sunset, civil twilight,
  nautical twilight, astronomical twilight, and moonlit night.
- Changes to coefficients should explain their source.
- Discontinuities across day / twilight / night transitions should be tested or
  visually justified.

Current limitation:

- The atmosphere is a compact renderer-oriented model, not a full spectral
  radiative-transfer simulation.
- Weather, clouds, local light pollution, and terrain obstruction are not
  modeled.

### Photometry and tone reproduction

Current implementation includes:

- magnitude-to-illuminance mapping;
- mesopic and scotopic adaptation behaviour;
- HDR accumulation;
- glare / PSF approximation;
- adaptive tonemapping.

Validation expectation:

- Monotonicity and representative value tests should protect brightness and
  colour response.
- Any display-facing heuristic should be documented as a rendering choice rather
  than an astronomical measurement.

Current limitation:

- Final output depends on display characteristics and is not colour-managed for
  every monitor.

### Planning helpers

Current implementation includes:

- evening window selection;
- rise / transit / set;
- twilight indicators;
- web planning table.

Validation expectation:

- Edge cases should be covered: always above horizon, always below horizon,
  high latitude, date boundaries, and twilight bands that do not occur.
- Future visibility scores, Moon-impact scores, recommended-object lists, and
  calendar exports should state which helper outputs they summarize.

Current limitation:

- Terrain horizon and weather constraints are not modeled.

### Open-cluster resolution (`V-53`)

Current implementation includes:

- a hand-curated open-cluster membership table
  (`crates/catalog/data/cluster_membership.csv`) joining HYG /
  Hipparcos IDs to a parent DSO id (`M44`, `M45`, `NGC869`, `NGC884`);
- a `resolve_as_member_field` predicate on `DeepSkyCatalog` that the
  renderer's marker pass consults to suppress disk-shaped markers over
  clusters whose stars are already in HYG.

Validation expectation:

- Pleiades 30' FOV: the 7 named bright Pleiades stars (Alcyone, Atlas,
  Electra, Maia, Merope, Taygeta, Pleione) must render at their
  catalog positions within 1' of the SIMBAD / Hipparcos reference.
- Marker suppression: every cluster tagged
  `resolve_as_member_field` must drop its overlay marker; every other
  DSO (galaxies, nebulae, planetary nebulae, globular clusters) must
  keep its marker.

Current numeric / structural pins:

- `pleiades_named_seven_positions_match_within_one_arcminute`
  (catalog crate, reads `data/hyg_v42.csv`) asserts the seven named
  Pleiades stars all sit within 1' of reference J2000 RA/Dec. The
  largest observed residual under HYG v4.2 is well below 0.1'.
- `deep_sky_markers_suppress_v53_resolved_clusters` (renderer crate)
  asserts the suppression policy fires for M44 / M45 / NGC 869 /
  NGC 884 and not for M31 / NGC 7000.
- `resolved_cluster_ids_match_v53_scope` (catalog crate) pins the
  shipped slice at exactly those four clusters.

Current limitation:

- The Double Cluster (NGC 869 + NGC 884) members reflect HYG v4.2's
  V ≤ 9 truncation, so the deep photometric core (V ~ 9–13) is not
  yet visible as resolved stars. The Cantat-Gaudin follow-up
  (`scripts/extract-cluster-membership.py --from-cantat-gaudin`) will
  expand the table once a deeper background-star catalog is wired.
- Hyades (Mel 25) is deferred: no V-42 marker today, so there is
  nothing to suppress.

### Catalog backend scaffold

Current implementation includes:

- `CatalogBackend` / `CatalogQuery` / `CatalogPage` scaffolding for future large
  catalogs;
- `HygCsvBackend` preserving HYG, HIP, and HD numeric identifiers CPU-side;
- source-side magnitude and row-limit filtering distinct from renderer limiting
  magnitude.

Validation expectation:

- Backend source labels must stay stable because session snapshots and future
  manifests depend on them.
- Identifier fields should be pinned when a backend starts preserving a new
  source ID.
- Large-catalog ingest should add tests for tile/LOD determinism, page cursor
  stability, and filtering boundaries.

Current limitation:

- Renderer buffers and host hover/copy flows do not yet expose preserved IDs;
  that remains tracked by `L-18`.

### Reproducible scenes and visual regression

Current implementation includes:

- schema-versioned JSON sessions that capture observer, time scales, view,
  overlays, projection/viewpoint, active corrections, atmosphere, catalog
  snapshot, and app version;
- deterministic scene presets for Tokyo evening, dark sky, noon, sunset, civil /
  nautical / astronomical twilight, moonlit night, a lunar eclipse aid, the
  2024-04-08 Mazatl\u00e1n total solar eclipse (`SolarEclipse`, V-51c), all-sky
  maps, and external galactic viewpoints;
- notebook examples in `examples/notebooks` that load the same JSON sessions,
  compare pinned Sun/Moon/planet tables, and optionally render via CLI;
- a validation/demo gallery generated from those presets by
  `scripts/render-validation-gallery.sh`;
- opt-in byte-for-byte visual-regression checks for representative renderer
  output where the host and CI environment can produce stable images.

Validation expectation:

- Preset names, inputs, expected model domains, generated tabular fixtures, and
  generated image dimensions should be documented.
- Visual diffs should use tolerances appropriate to GPU and platform variation;
  they should catch meaningful regressions without implying pixel-perfect
  portability across all hardware.
- Any notebook table fixture change should include the regenerated CSV and a
  reason the astronomy-model output changed.
- Any screenshot used as evidence in a PR should identify the scene preset or
  session file that produced it.

Current limitation:

- Screenshot regression is intentionally opt-in rather than part of default
  `make ci` because adapter/driver differences can change readback bytes. PRs
  should still regenerate and inspect gallery images for renderer-visible
  changes, or explain why doing so was not practical.

## Citation and standards traceability

When using validation output in a paper, teaching material, or issue report,
follow [`docs/citation.md`](docs/citation.md): cite the archived release or
exact commit, attach the JSON session or preset name, and name any relevant
approximation from [`docs/standards-compliance.md`](docs/standards-compliance.md).

## External comparison targets

Use these as comparison targets when practical:

- IAU / SOFA algorithms for time and coordinate standards;
- JPL Horizons for apparent solar-system positions;
- published photometry and atmosphere references named in `ROADMAP.md`;
- known catalog source documentation for star data and identifiers.

When adding a comparison, record:

- external tool / dataset version;
- input time scale;
- observer location;
- target body or star;
- output coordinate frame;
- tolerance;
- date when the reference value was captured.

## Testing style

Prefer small tests that pin a specific contract:

```rust
#[test]
fn j2000_utc_maps_to_expected_julian_date() {
    let ts = TimeScales::from_unix_seconds(946728000.0);
    assert!((ts.jd_utc - 2_451_545.0).abs() < 1e-9);
}
```

For approximate models, avoid over-tight tolerances that imply false precision.
The test should catch regressions while respecting the intended model accuracy.

## Documentation requirements for new scientific models

A PR adding or replacing a scientific model should update docs with:

- model name;
- citation or source;
- valid domain;
- expected error or approximation level;
- tests added;
- affected hosts;
- whether the model is visual-only or intended for numerical astronomy.
