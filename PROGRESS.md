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
  paths (`V-37`), full-sky projections (`V-40`), out-of-Earth galactic and
  custom external viewpoints (`V-41`, `V-44`), telescope eyepiece simulation
  (`V-43`), and the deep-sky overlay with Messier objects plus the bright
  NGC / IC subset (`V-42`).
- **Library track** — IAU-grade time / precession / nutation / aberration /
  proper motion (`L-01`–`L-05`), planning helpers (`L-07`, `L-08`),
  schema-versioned JSON sessions (`L-10`, `L-11`), deterministic scene
  presets (`L-12`), notebook reproducibility examples (`L-13`), catalog
  backend scaling scaffold (`L-16`), validation / demo gallery (`L-27`),
  citation metadata (`L-25`), standards-compliance document (`L-26`), and
  the data provenance manifest (`L-15`).

Still open:

- **Visual track** — dark-sky realism gaps (`V-24`–`V-28`), daylight model
  upgrade and site-specific brightness (`V-38`–`V-39`), niche visual
  features (`V-45`–`V-50`), rare phenomena (`V-47`–`V-49`). A follow-up PR will add a runtime
  streaming backend for the full ~14,000-entry OpenNGC catalogue on top of
  the embedded `V-42` subset shipped here.
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
2. `localStorage["stars:locale"]` (the user's most recent manual choice);
3. `navigator.language` / `navigator.languages` prefix match;
4. fallback to `en`.

A language switcher in the Settings popover persists the choice through
`localStorage`, and the active locale is mirrored onto `<html lang="…">` so
assistive tech sees the right language. English remains the source of truth
for the key set; missing Japanese keys fall back to English at lookup time.

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
