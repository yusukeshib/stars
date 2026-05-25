# Progress

This document is the implementation log for `stars`. It records what has been
implemented well enough to count as shipped. The forward-looking plan lives in
[`ROADMAP.md`](ROADMAP.md).

A feature belongs here when it is implemented, documented in the relevant code
or docs, covered by tests where numerical output matters, and wired into the
relevant host applications.

## Summary

Implemented phase groups:

- Phase 1 core educational overlays and text labels.
- Phase 1' physical dark-sky visual pipeline.
- Phase 2 time systems, apparent-place corrections, atmosphere, solar-system
  bodies, and planning UI.
- Phase 3 schema-versioned JSON sessions, deterministic scene presets,
  notebook reproducibility examples, a validation/demo gallery workflow,
  citation metadata, and standards-compliance notes.
- Phase 4 full-sky projections and external galactic viewpoint.

Still open:

- Remaining Phase 3 research / platform features.
- Remaining Phase 4 advanced visual features.

## Phase 1 — Educational planetarium

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

## Phase 1' — Physical dark-sky visual pipeline

Phase 1' is implemented as the dark-sky visual realism layer. It is orthogonal
to positional precision: the goal is that the sky looks physically plausible to
a dark-adapted observer.

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
- per-channel extinction coefficients;
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

## Phase 2 — Observation planning and positional trust

Phase 2 is implemented in the roadmap table. It provides the default precision
and physical atmosphere layer expected of the current viewer.

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

## Phase 3 — Reproducibility and platform baseline

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

### Citation and standards baseline

Implemented the first citable-platform baseline for Phase 3:

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

### Scene presets, notebook examples, and validation gallery

Implemented deterministic Phase 3 scene presets for reproducible demos,
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

## Phase 4 — Advanced visual features

### Full-sky projections

Implemented the first Phase 4 visual feature: selectable screen projections.
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

Implemented a Phase 4 external camera mode for viewing the local Milky Way from
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

Generalized the fixed Phase 4 galactic viewpoint into a host-selectable external
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

Implemented the Phase 4 telescope eyepiece model. Hosts can enable an optical
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
