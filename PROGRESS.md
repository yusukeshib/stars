# Progress

This document is the implementation log for `stars`. It records what has been
implemented well enough to count as shipped. The forward-looking plan lives in
[`ROADMAP.md`](ROADMAP.md).

A feature belongs here when it is implemented, documented in the relevant code
or docs, covered by tests where numerical output matters, and wired into the
relevant host applications.

## Summary

Implemented phase groups:

- Phase 1 core educational overlays, except text labels.
- Phase 1' physical dark-sky visual pipeline.
- Phase 2 time systems, apparent-place corrections, atmosphere, solar-system
  bodies, and planning UI.

Still open:

- Phase 1 text labels.
- Phase 3 research / platform features.
- Phase 4 advanced visual features.

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
- Web settings UI for overlay selection.
- Web localStorage persistence for observer and view state.

Primary implementation areas:

- `apps/cli`
- `apps/viewer`
- `apps/web/frontend`
- `crates/common`

### Remaining Phase 1 work

Text rendering is still open:

- star / planet / constellation labels;
- N / E / S / W and degree labels.

Those should probably share one font atlas and label placement system.

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
- schema-versioned `starsSession=2` session URLs.

Primary implementation areas:

- `crates/astronomy/src/planning.rs`
- `apps/web/frontend`

## Documentation progress

The documentation has been split into purpose-specific files:

- `README.md` / `README.ja.md` for entry points;
- `ROADMAP.md` for forward plan;
- `PROGRESS.md` for implementation log;
- `ARCHITECTURE.md` for crate boundaries and host integration;
- `CONTRIBUTING.md` for development process;
- `VALIDATION.md` for scientific validation policy;
- `DATA_SOURCES.md` for data provenance.

## Next implementation log entries

When new work lands, add a short entry here with:

1. what changed;
2. why it counts as complete;
3. where the implementation lives;
4. what tests or validation pin the behaviour;
5. which hosts are wired, if applicable.
