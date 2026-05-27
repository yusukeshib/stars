# Roadmap

This document is the planning source of truth for `stars`: what has shipped,
what remains open, and what each track is supposed to buy. The implementation
history is tracked separately in [`PROGRESS.md`](PROGRESS.md).

The aim is for `stars` to be useful both as a casual night-sky viewer and as a
piece of software that astronomers, educators, and researchers can defend
using. Work is organised along **two orthogonal tracks** that match those two
audiences:

| Track | Theme | What it buys |
|---|---|---|
| **V** — Visual | Sky rendering | The sky *looks* like the sky a dark-adapted human would see, and you can identify what you're looking at |
| **L** — Library / platform | Numerical engine + reach | Positions, time, and ephemerides match published standards, and the engine ships as a citable, embeddable, reproducible library |

Items in track **V** affect what pixels appear on screen for the *casual
observer*: overlays, photometric and perceptual modelling, atmosphere, glare,
projections, instrument simulation. Items in track **L** are added for
*researchers, educators, and downstream software*: corrections, ephemeris
precision, sessions, manifests, bindings, citations. The two tracks are
independent — either can land in any order, and neither blocks the other.

Item IDs encode their track: `V-NN` (Visual) or `L-NN` (Library). Each item is
its own micro-design block with item statement, scientific basis, references,
implementation scope, tests / validation, host coverage, and dependencies on
other items.

Earlier revisions of this document used phase numbers (Phase 1, Phase 1',
Phase 2, Phase 3, Phase 4). The git history retains the old PR labels; the
current table below is the source of truth.

---

## Visual track — scope

The Visual track covers two halves of "the sky looks like the sky":

1. **Identification** — overlays, constellation lines and boundaries, star /
   planet / constellation / cardinal / degree labels.
2. **Physical realism** — HDR rendering, eye PSF and glare, atmospheric
   extinction, diffuse night-sky background, mesopic / scotopic adaptation,
   tone reproduction, scintillation, dispersion, earthshine, Belt of Venus,
   spectral airglow, daylight scattering, twilight composition, Sun / Moon /
   planet disks, projections, external viewpoints, eyepiece simulation, deep
   sky overlays, telescope optical artifacts, Galactic structural model, rare
   phenomena (meteors / aurora / comets), light pollution, and colour-managed
   output.

### Atmosphere-rendering scope

The dark-sky atmosphere (stellar extinction, night-sky background, zodiacal
light, airglow, dust, glare, human eye adaptation) and the sunlit atmosphere
(blue daytime sky, sunset reddening, twilight colour gradients) cooperate
through one shared atmospheric optical-depth model (`V-37`): stellar
extinction at night and Rayleigh / Mie / ozone scattering by day read the same
(β, α, DU) state, so reddening at the horizon is consistent across day and
night.

### Atmosphere implementation ladder

To avoid conflating prototypes with completion, the atmosphere visual work
is split into independently shippable rungs:

1. **Input plumbing** — Sun/Moon apparent directions, radii, phase, and host
   controls available to the renderer. ✅
2. **Renderable bodies** — Sun/Moon disks drawn from those inputs without
   polluting the star catalogue. ✅ (low-precision ephemerides; `L-06`
   upgrades).
3. **Daylight model** — daylight sky colour driven by a cited model over its
   valid domain. ✅ (Preetham via `V-32`; replaced by Hošek-Wilkie 2012
   in `V-38`).

4. **Twilight model** — sun-below-horizon sky brightness from a real radiance
   model, continuous in time and direction. ✅ (zenith curve via `V-33`;
   `V-27` adds the azimuthally-resolved Belt of Venus / Earth-shadow band).
5. **Validation** — noon, sunset, civil / nautical / astronomical twilight,
   and moonlit-night reference scenes pinned by tests / screenshots. ✅
6. **Unified extinction** — one (β, α, DU) optical-depth state shared by the
   star-extinction path and the daylight scattering shader. ✅ (`V-37`).
7. **Site-specific brightness** — observer-side light-pollution selector
   (Bortle / SQM / Falchi atlas). ⬜ (`V-39`).

---

## Library track — scope

The Library track covers the parts of the engine that are not about screen
appearance but about *trust* and *reach*:

1. **Numerical correctness** — time scales, precession, nutation, aberration,
   proper motion, high-precision ephemerides.
2. **Observation planning helpers** — rise / transit / set, twilight band
   times, Moon-impact and visibility scoring.
3. **Reproducibility surface** — JSON sessions, scene presets, notebook
   examples, data provenance manifest, public demo gallery.
4. **Catalog data layer** — backend trait, large-catalog ingest, identifier
   preservation, external-service deep links, variable-star metadata.
5. **Bindings and hosts** — Python bindings, headless server mode.
6. **Education and accessibility** — guided tour content, WCAG compliance.
7. **Citation and standards traceability** — CITATION.cff / Zenodo metadata,
   standards-compliance document, validation / demo gallery.

Library items often *do* shift on-screen pixels (precession moves star labels
~50″/yr; DE440 moves the Sun slightly), but they exist for numerical or
reproducibility reasons, not visual realism. Their visible effect is a
side-effect, not the motivation.

---

## Current focus

The Visual track is at "naked-eye physical realism is mostly there" — the
remaining items are realism polish (`V-25`–`V-28`; `V-24` scintillation has
shipped), site-specific brightness
(`V-39`; the unified (β, α, DU) state it depends on is in place via
`V-37`, and the daylight model upgrade `V-38` has shipped), niche visual
features (`V-45`–`V-50`), and rare phenomena (`V-47`–`V-49`).

**High priority next:** the visual-richness gaps `V-51`–`V-56` (eclipses /
occultations, planetary rings and moons, resolved star clusters, double
stars, artificial satellites, and object search / GoTo / info panel) —
these surface existing engine capability that the current UI hides. The
first three slices of `V-51` (common occultation primitives, the
general N≤16 occluder uniform array, and the solar-eclipse renderer
path — `V-51a` + `V-51b` + `V-51c`) have shipped; `V-51d`/`e`/`f` (lunar
occultation + planetary transit + mutual planetary occultation) are
next.

The Library track is at "amateur-grade is shipped" — the remaining items are
DE440-class ephemerides (`L-06`), large catalog ingest (`L-17`), bindings and
headless server (`L-21`, `L-22`), planning polish (`L-09`), variable-star
library (`L-20`), and education / accessibility (`L-23`, `L-24`).

A row is `✅ done` only when the model named in its references is implemented,
documented, tested, and wired into all relevant hosts.

---

## Status at-a-glance

Legend: ✅ done, ⏳ next, ⬜ open.

### Visual track

| ID | Item | Status |
|---|---|---|
| `V-01` | Sky overlays library | ✅ |
| `V-02` | Seven reference circles | ✅ |
| `V-03` | CLI overlay flags | ✅ |
| `V-04` | Desktop viewer overlay flags | ✅ |
| `V-05` | Web HUD + settings modal | ✅ |
| `V-06` | Web localStorage persistence | ✅ |
| `V-07` | Web overlay toggles | ✅ |
| `V-08` | Galactic equator overlay | ✅ |
| `V-09` | Constellation lines | ✅ |
| `V-10` | IAU / Delporte constellation boundaries | ✅ |
| `V-11` | Star / planet / constellation labels | ✅ |
| `V-12` | Cardinal and degree labels | ✅ |
| `V-13` | Photometric zeropoint | ✅ |
| `V-14` | Mesopic chromatic-fidelity weight | ✅ |
| `V-15` | Purkinje-shifted scotopic desaturation | ✅ |
| `V-16` | HDR render target | ✅ |
| `V-17` | Eye PSF / glare (Spencer + corona) | ✅ |
| `V-18` | Atmospheric extinction (Kasten-Young + Schaefer) | ✅ |
| `V-19` | Diffuse sky background (ISL + DGL) | ✅ |
| `V-20` | Sky tone reproduction (adaptive Reinhard) | ✅ |
| `V-21` | Zodiacal light + airglow + interstellar dust | ✅ |
| `V-22` | Per-fragment rod/cone tonemap | ✅ |
| `V-23` | Catalogue colour pipeline (B−V → T → blackbody → sRGB) | ✅ |
| `V-24` | Atmospheric scintillation | ✅ |
| `V-25` | **Differential atmospheric dispersion** | ⬜ |
| `V-26` | **Lunar earthshine** | ⬜ |
| `V-27` | **Belt of Venus + Earth-shadow band** | ⬜ |
| `V-28` | **Spectral airglow decomposition** | ⬜ |
| `V-29` | Atmospheric refraction | ✅ |
| `V-30` | Sun and Moon apparent topocentric state | ✅ |
| `V-31` | Solar / lunar illuminants | ✅ |
| `V-32` | Sunlit atmospheric scattering (Preetham) | ✅ |
| `V-33` | Twilight / day-night blend | ✅ |
| `V-34` | Atmosphere controls in CLI / viewer / web | ✅ |
| `V-35` | Planets (Mercury → Neptune) | ✅ |
| `V-36` | Moon phase + Earth-shadow aid | ✅ |
| `V-37` | Unified spectral extinction (β / α / DU) | ✅ |
| `V-38` | Hošek-Wilkie daylight sky model | ✅ |
| `V-39` | **Light pollution / Bortle map** | ⬜ |
| `V-40` | Full-sky projections (Mollweide / Aitoff / Hammer) | ✅ |
| `V-41` | Out-of-Earth galactic-north viewpoint | ✅ |
| `V-42` | Deep-sky overlay (Messier + bright NGC / IC subset) | ✅ |
| `V-43` | Telescope eyepiece simulation | ✅ |
| `V-44` | Custom external viewpoint origin | ✅ |
| `V-45` | **Telescope-side optical artifacts** | ⬜ |
| `V-46` | **Galactic structural model for external viewpoints** | ⬜ |
| `V-47` | **Meteor shower display** | ⬜ |
| `V-48` | **Aurora display** | ⬜ |
| `V-49` | **Comet rendering** | ⬜ |
| `V-50` | **Output colour management (sRGB / P3 / Rec.2020)** | ⬜ |
| `V-51` | **Unified eclipse / occultation pass** (`a` + `b` + `c` done; `d`/`e`/`f` open) | ⏳ |
| `V-52` | **Planetary rings and moons (Saturn / Galilean / Titan)** | ⬜ |
| `V-53` | **Resolved star clusters (Pleiades, Hyades, …)** | ⬜ |
| `V-54` | **Double / binary star resolution** | ⬜ |
| `V-55` | **Artificial satellites (TLE / SGP4)** | ⬜ |
| `V-56` | **Object search, GoTo, and info panel** | ⬜ |

### Library track

| ID | Item | Status |
|---|---|---|
| `L-01` | Time systems (UTC / UT1 / TAI / TT / TDB) | ✅ |
| `L-02` | IAU 2006 precession | ✅ |
| `L-03` | Compact nutation | ✅ |
| `L-04` | Annual aberration | ✅ |
| `L-05` | Stellar proper motion | ✅ |
| `L-06` | DE440 / VSOP87 ephemeris upgrade | ⬜ |
| `L-07` | Rise / transit / set tables | ✅ |
| `L-08` | Twilight indicators | ✅ |
| `L-09` | Observation-planning polish | ⬜ |
| `L-10` | Session URL encoding | ✅ |
| `L-11` | Sharable JSON sessions | ✅ |
| `L-12` | Scene presets | ✅ |
| `L-13` | Notebook examples | ✅ |
| `L-14` | Public demo gallery | ⬜ |
| `L-15` | Data provenance manifest | ✅ |
| `L-16` | Catalog backend scaling design | ✅ |
| `L-17` | Hipparcos / Tycho-2 / Gaia DR3 ingest | ⬜ |
| `L-18` | Identifier preservation through the renderer | ⬜ |
| `L-19` | SIMBAD / VizieR deep links | ⬜ |
| `L-20` | Variable star light curves | ⬜ |
| `L-21` | Python bindings (PyO3) | ⬜ |
| `L-22` | Headless server mode | ⬜ |
| `L-23` | Guided education mode | ⬜ |
| `L-24` | Accessibility pass | ⬜ |
| `L-25` | `CITATION.cff` + Zenodo DOI | ✅ |
| `L-26` | Standards-compliance document | ✅ |
| `L-27` | Validation / demo gallery | ✅ |

---

# Visual track

## Identification overlays

### `V-01` Sky overlays library — ✅ done

**Item.** A renderer-side library that draws arbitrary great circles, small
circles, line segments, and grid families on top of the star pass without
reading or writing the star buffer. Generalises every later overlay.

**Implementation.** `crates/renderer/src/overlay.rs`. Hosts pass an
`OverlayConfig`; the renderer emits the vertex buffers and shader bindings
needed by the dedicated overlay pass.

**Tests / validation.** Unit tests on the geometry helpers; visual gallery
covers all seven reference circles.

---

### `V-02` Overlay layers — seven reference circles — ✅ done

**Item.** Horizon, cardinal markers, alt-az grid, equatorial grid, ecliptic,
celestial equator, and meridian — selectable per host.

**Scientific basis.** Pedagogical convention: these are the seven coordinate
references every introductory astronomy text overlays on the sky. Constants
(ecliptic obliquity, celestial-equator pole) come from IAU 2006.

**Implementation.** All seven shipped in PR #1 via `renderer::overlay`.

**Tests / validation.** Renderer unit tests on each geometry generator;
deterministic scene presets cover each overlay in the validation gallery.

---

### `V-03` CLI overlay flags — ✅ done

**Item.** `--overlays`, `--no-overlays`, `--grid-step-deg`, and
`--overlay-opacity` give the CLI a single argument surface to drive every
overlay layer end-to-end.

**Implementation.** `apps/cli`. Flags map into `OverlayConfig`; the same
struct is serialised in JSON sessions so CLI / viewer / web stay in sync.

**Tests / validation.** Integration tests cover `--no-overlays`,
`--grid-step-deg`, and round-trip of `OverlayConfig` through the session
schema.

---

### `V-04` Desktop viewer overlay flags — ✅ done

**Item.** Parity between the desktop viewer and the CLI: every CLI overlay
flag has a viewer equivalent (keybinding or menu toggle).

**Implementation.** `apps/viewer`. The viewer reuses the CLI's argument
parser and the same `OverlayConfig`.

**Tests / validation.** Manual parity matrix in `docs/scene-presets.md`;
session-roundtrip tests cover the shared schema.

---

### `V-05` Web HUD redesign — ✅ done

**Item.** Single gear button opening an organised modal settings panel,
replacing the originally-scattered floating controls. Keyboard- and mouse-
accessible.

**Implementation.** `apps/web/frontend`. Settings panel groups: observer,
time, overlays, atmosphere, projection, viewpoint, eyepiece.

**Tests / validation.** Component-level tests for the settings modal;
`L-24` accessibility pass will pin keyboard / screen-reader support.

---

### `V-06` localStorage persistence — ✅ done

**Item.** Observer and view state survive page reloads via `localStorage`,
so casual users don't need to re-enter their location each visit.

**Implementation.** `apps/web/frontend` writes the same JSON session schema
into `localStorage` under a versioned key, so future schema bumps don't
corrupt older saves.

**Tests / validation.** Browser test covers load → mutate → reload →
identical render.

---

### `V-07` Web overlay toggles — ✅ done

**Item.** Settings modal exposes every overlay flag from `V-03`, so the web
UI is at functional parity with the CLI.

**Implementation.** `apps/web/frontend`. Each toggle drives the same
`OverlayConfig` writer used by `localStorage` persistence.

**Tests / validation.** Manual matrix; gallery includes overlay-on and
overlay-off web screenshots.

---

### `V-08` Galactic equator overlay — ✅ done

**Item.** Eighth reference circle: the IAU 1958 galactic equator drawn in
the same line pipeline as the ecliptic.

**Scientific basis.** IAU 1958 galactic pole at α=192.860°, δ=27.128° in
J2000. The transform between equatorial and galactic coordinates is
documented in `crates/astronomy`.

**Implementation.** `renderer::overlay` (geometry shared with ecliptic
generator).

**Tests / validation.** Unit test pins the galactic-pole transform; gallery
includes a galactic-equator-on scene.

---

### `V-09` Constellation lines — ✅ done

**Item.** Modern western stick figures rendered as line segments between
named catalogue stars.

**Source.** `crates/renderer/data/constellation_lines.csv`, derived from
the BSD-licensed `d3-celestial` line data, compacted by the renderer build
script.

**Implementation.** `renderer::constellations` + `renderer::overlay`.

**Tests / validation.** Build-script invariant: every line endpoint
references a HYG star ID present in the embedded catalog.

---

### `V-10` Constellation boundaries — ✅ done

**Item.** IAU / Delporte 1930 constellation boundary polygons drawn as
great-circle arcs.

**Source.** `crates/renderer/data/constellation_boundaries.csv`, derived
from CDS VI/49 (Delporte 1930) B1875 boundary vertices precessed to J2000
and compacted by the renderer build script.

**Reference.** Delporte, E. 1930, *Délimitation Scientifique des
Constellations* (IAU / Cambridge).

**Implementation.** `renderer::constellations` + `renderer::overlay`.

**Tests / validation.** Total boundary closure check (every region closes);
gallery scene compares to Stellarium's same view.

---

### `V-11` Star / planet / constellation labels — ✅ done

**Item.** Proper names + Bayer / Flamsteed designations for the top ~50
stars, planet names, and constellation names placed without overlap.

**Implementation.** Built-in bitmap font atlas + a screen-space label-
placement pass. Top-50 anchors are generated from HYG v4.2; solar-system
labels use renderer apparent Sun / Moon / planet positions.
`renderer::text` + `renderer::overlay`.

**Tests / validation.** Snapshot tests on the placement pass; gallery
includes labelled and unlabelled scenes.

---

### `V-12` Cardinal and degree labels — ✅ done

**Item.** N / E / S / W cardinal labels and optional alt-az degree ticks
drawn from the same text atlas.

**Implementation.** `renderer::text` + `apps/{cli,viewer,web}`.

**Tests / validation.** Gallery scene includes a degree-labelled alt-az
grid.

---

## Dark-sky physical realism

### `V-13` Photometric zeropoint — ✅ done

**Item.** Magnitude-to-illuminance conversion that puts the entire pipeline
on physical (lux) units, so every downstream perceptual model can be
defended against the literature rather than tuned by eye.

**Scientific basis.** `E_v = 10^(−0.4 · (m_V + 13.99))` lux. A magnitude-0
star produces ≈ 2.54 × 10⁻⁶ lux outside the atmosphere; the constant 13.99
is Schaefer 1990's Eq. 1.

**Reference.** Schaefer, B. E. 1990, PASP 102, 212, Eq. 1 and Table 1.

**Implementation.** `astronomy::photometry::magnitude_to_illuminance_lux`.

**Tests / validation.** Pinned value tests at m=0, −1, +6 against
hand-computed expected illuminance.

---

### `V-14` Mesopic chromatic-fidelity weight — ✅ done

**Item.** Log-linear blend over the CIE 191:2010 mesopic range
(0.005–5 cd/m²), applied per-star so only stars bright enough to stimulate
cones retain their B−V colour. Faint stars converge to the rod-weighted
desaturation handled by `V-15`.

**Reference.** CIE 191:2010, *Recommended System for Mesopic Photometry
Based on Visual Performance*.

**Implementation.** `astronomy::photometry::mesopic_chromatic_weight`.

**Tests / validation.** Unit tests at the regime endpoints (0.005, 0.5,
5 cd/m²); the same constants are mirrored in the tonemap shader.

---

### `V-15` Purkinje-shifted scotopic desaturation — ✅ done

**Item.** Faint stars collapse toward a rod-weighted (~507 nm peak) grey
rather than a flat luma. Implements the perceptual blue-shift / red-loss
characteristic of dark-adapted vision.

**Reference.** CIE 1951 V'(λ); Bowmaker & Dartnall 1980, J. Physiol. 298,
501.

**Implementation.** `astronomy::photometry` + `crates/catalog/src/color.rs`
collaborate so the catalogue-colour pipeline already returns a perceptually
weighted RGB triple at faint magnitudes.

**Tests / validation.** Unit test pins a B-type star's perceived colour at
m=7 vs. m=2 (must desaturate).

---

### `V-16` HDR render target — ✅ done

**Item.** Replace the 8-bit sRGB attachment with `Rgba16Float` so faint-star
contributions accumulate instead of being crushed by the discard cutoff.

**Reference.** Reinhard et al. 2002, SIGGRAPH '02, "Photographic Tone
Reproduction for Digital Images".

**Implementation.** `crates/renderer/src/tonemap.rs`, `pipeline.rs`,
`renderer.rs`.

**Tests / validation.** Renderer integration test renders an m=8 star next
to an m=0 star and asserts the m=8 contribution is non-zero in the HDR
buffer.

---

### `V-17` Eye PSF / glare — ✅ done

**Item.** 3-component Spencer human PSF (sharp Gaussian core, 1/r³
lenticular halo, 1/r² corneal halo) plus a 4-point ciliary corona for
bright sources.

**References.**
- Spencer, G., Shirley, P., Zimmerman, K., Greenberg, D. P. 1995, SIGGRAPH
  '95, "Physically-Based Glare Effects for Digital Images".
- Ritschel, T. et al. 2009, Eurographics, "Temporal Glare: Real-Time
  Dynamic Simulation of the Scattering in the Human Eye".

**Implementation.** `crates/renderer/src/shaders/star.wgsl`.

**Tests / validation.** Pinned PNG of Vega + Sirius gallery; ratio between
core and halo radii matches the Spencer constants.

**Dependencies.** `V-45` extends this with a telescope-instrument PSF
convolved on top of Spencer, so eyepiece mode shows Airy / spider / SCT
patterns.

---

### `V-18` Atmospheric extinction — ✅ done

**Item.** Kasten-Young airmass × per-channel Hardie 1962 / Schaefer 1993
extinction coefficients, applied per-star in the vertex shader so the
photometric reduction is consistent with the visual literature.

**References.**
- Kasten, F., Young, A. T. 1989, Applied Optics 28, 4735.
- Schaefer, B. E. 1993, Vistas in Astronomy 36, 311.
- Hardie, R. H. 1962, *Photoelectric Reductions* (ApJ Suppl. compilation).

**Implementation.** `astronomy::photometry::airmass_kasten_young`,
`renderer::Atmosphere`, `crates/renderer/src/shaders/star.wgsl`.

**Tests / validation.** Unit tests: Kasten-Young airmass at altitudes
90°, 30°, 5°, 0° matches the published curve; per-channel coefficients
at altitude 30° reproduce Hardie's sea-level numbers within 0.02
mag/airmass.

**Dependencies.** `V-37` unifies this with the daylight scattering shader
so a single (β, α, DU) state drives both stellar reddening and daylight
sky colour.

---

### `V-19` Diffuse sky background — ✅ done

**Item.** Integrated-starlight (ISL) + diffuse-galactic-light (DGL)
analytic fit to Leinert et al. 1998 §6, evaluated per fragment in galactic
coordinates as a fullscreen pass. This is what makes the Milky Way visible
without resorting to a bitmap texture.

**References.**
- Leinert, Ch. et al. 1998, A&AS 127, 1.
- Roach, F. E., Megill, L. R. 1961, ApJ 133, 228.

**Implementation.** `astronomy::skyglow`,
`crates/renderer/src/shaders/skyglow.wgsl`.

**Tests / validation.** Unit test pins the model at the Galactic pole and
the Galactic centre against Leinert §6 reference points.

---

### `V-20` Sky tone reproduction — ✅ done

**Item.** Adaptive Reinhard 2002 §3.3 keyed operator with scene-luminance
reduction + CIE 191:2010 mesopic regime split. Ferwerda 1996 TVI functions
motivate the photographic-key selection (cone Zone V at high luminance,
rod Zones II–III at low luminance). The tonemap pass applies per-fragment
rod / cone separation with a compact Pattanaik-style local adaptation
luminance.

**References.**
- Reinhard, E. et al. 2002, SIGGRAPH '02, §3.2 / 3.3.
- Ferwerda, J. A. et al. 1996, SIGGRAPH '96, §3 (TVI Eqs. 1–2).
- CIE 191:2010 (mesopic blend bounds 0.005–5 cd/m²).
- Adams, A. 1948, *The Negative*, ch. 4 (Zone System; photopic key
  derivation).

**Implementation.** `astronomy::photometry::{cone_tvi_log10,
rod_tvi_log10, hdr_flux_to_luminance_cd_m2}` +
`crates/renderer/src/shaders/{luminance,tonemap}.wgsl`.

**Tests / validation.** Pinned-value unit tests on the TVI functions;
gallery includes a high-luminance and a low-luminance reference scene.

---

### `V-21` Zodiacal light + airglow + interstellar dust — ✅ done

**Item.** Sun-relative zodiacal-light band + antisolar gegenschein, dark-
site airglow floor, and SFD-inspired analytic dust extinction, summed in
S10 flux units in both the Rust reference model and the skyglow shader.

**References.**
- Leinert, Ch. et al. 1998 §5 (zodiacal light), §7 (airglow).
- Schlegel, D. J., Finkbeiner, D. P., Davis, M. 1998, ApJ 500, 525 (dust).

**Implementation.** `astronomy::skyglow`,
`crates/renderer/src/shaders/skyglow.wgsl`.

**Tests / validation.** Unit tests pin the zodiacal band and gegenschein
against Leinert §5 reference points; dust extinction at the Galactic plane
matches SFD's anchor values within the documented tolerance.

**Dependencies.** `V-28` splits the single airglow floor constant into its
O I / Na D / OH spectral components.

---

### `V-22` Per-fragment rod / cone tone reproduction — ✅ done

**Item.** Tonemap computes fragment-local adaptation luminance, selects rod
/ cone response from the local CIE 191 mesopic state, and feeds the result
through the Reinhard keyed operator.

**References.**
- Pattanaik, S. N. et al. 1998, SIGGRAPH '98 (multiscale model).
- Ferwerda, J. A. et al. 1996, SIGGRAPH '96.
- Durand, F., Dorsey, J. 2002 (edge-aware local-adaptation refinement).

**Implementation.** `crates/renderer/src/shaders/tonemap.wgsl`.

**Tests / validation.** Pinned-output scene comparing globally adapted vs.
locally adapted output.

---

### `V-23` Catalogue colour pipeline — ✅ done

**Item.** B−V → T_eff → blackbody spectrum → CIE 1931 XYZ → sRGB. Replaces
the original piecewise-polynomial fit so the photopic input to the mesopic
blend is physically calibrated.

**References.**
- Ballesteros, F. J. 2012, EPL 97, 34008, Eq. 14 (B−V → T_eff).
- Wyman, C., Sloan, P.-P., Shirley, P. 2013, JCGT 2(2) (compact analytic
  CIE 1931 colour-matching-function fit).

**Implementation.** `crates/catalog/src/color.rs`.

**Tests / validation.** Pinned RGB outputs at B−V = −0.3 (B-type, blue),
0.0 (A0V, white), 0.65 (G2V solar, faint yellow-white), 1.5 (M-type, red).

---

### `V-24` Atmospheric scintillation — ✅ done

**Item.** Time-varying intensity (and, secondarily, colour) modulation on
each star, with variance growing toward the horizon and damping at high
observatory altitudes. The unaided eye sees ~Hz-band twinkling on bright
low-altitude stars; this is the visual signature the current dark-sky
pipeline is missing because every frame's star flux is deterministic.

**Scientific basis.** Naked-eye scintillation is a weak-turbulence problem
solved by Tatarski / Roddier theory. For an unaided 7 mm pupil the
relative intensity variance is

```
σ_I² ≈ 10.66 · sec(z)^3 · exp(−2 h_obs / H_atm) · D_pupil^(−7/3) · Σ_C
```

where `H_atm ≈ 8000 m` and `Σ_C` is the line-of-sight-integrated Cn²
profile collapsed to a single calibrated column constant (we pick K so the
σ at sea level / airmass 1 matches published amateur-site measurements).
Temporal spectrum follows Dravins et al. 1997 / 1998: low-pass with corner
~10–30 Hz at low altitudes, falling off at the zenith. Colour scintillation
appears as a small per-channel phase offset in the same noise field, with
the chromatic amplitude factor of Dravins 1998.

**References.**
- Young, A. T. 1967, AJ 72, 747 ("Photometric error analysis. VIII.
  Scintillation of stars").
- Dravins, D., Lindegren, L., Mezey, E., Young, A. T. 1997, PASP 109, 173
  (Part I — statistical distributions and temporal properties).
- Dravins, D. et al. 1997, PASP 109, 725 (Part II).
- Dravins, D. et al. 1998, PASP 110, 610 (Part III — colour scintillation).
- Roddier, F. 1981, *Progress in Optics* 19, 281.

**Implementation.**
- `crates/astronomy/src/scintillation.rs`:
  `intensity_variance(altitude_rad, h_obs_m, pupil_mm, c_n2_scale) ->
  (sigma_sq, corner_hz)`. The Young 1967 prefactor (10.66) is bundled
  with a `CALIBRATION` constant chosen so the default `c_n2_scale = 1.0`
  reproduces Dravins 1997 amateur-site σ ≈ 4 % at the zenith for a 7 mm
  pupil at sea level. The Hufnagel-Valley boundary-layer-dominated Cn²
  scale height (`H_turb = 4000 m`) is used instead of the 8 km pressure
  scale height because the bulk of scintillation-relevant turbulence
  sits in the surface layer + lower troposphere; 8 km would underpredict
  the observed factor-of-5–10 drop between sea-level and ~4 km
  observatories.
- `crates/renderer/src/shaders/star.wgsl`: a `scintillation_params: vec4`
  uniform carries `(σ²_zenith, f_corner_zenith, seed, t_seconds_mod_day)`.
  A PCG-hashed per-star, time-bin-interpolated noise field (corner =
  `f_corner_zenith / √sec z`) modulates the post-extinction RGB flux by
  `(1 + σ · n)`. Three samples at slightly offset times give the Dravins
  1998 colour-shimmer.
- `crates/renderer/src/camera.rs`: new `Scintillation { enabled,
  c_n2_scale, seed }` field on `Camera`. Zeroed automatically when the
  external galactic viewpoint is active or when `Atmosphere::OFF` is set.
- Time source: `t = fract(jd_ut1) × 86400` so two renders of the same
  session at the same simulated UT1 produce bit-identical pixels.
- `crates/common`: `ScintillationConfig` + `scintillation_from_args`
  helper; session schema bumped to v4 with the new `scintillation` block.
  CLI / viewer flags: `--no-scintillation`, `--scintillation-scale`,
  `--scintillation-seed`. The web frontend mirrors the same state in
  `observer.ts` / `session.ts` / `storage.ts`, with the WASM binding
  exposed as `StarView.set_scintillation`.

**Tests / validation.**
- `astronomy::scintillation` unit tests pin: default scale matches
  amateur-site σ ≈ 4 %; σ²(airmass = 5) > 10× σ²(airmass = 1);
  σ²(4 km observer) < σ²(sea level) by > 5×; larger pupils crush σ² via
  the D⁻⁷ᐟ³ aperture-averaging exponent; corner frequency follows
  `1/√sec z`; disabled scale and NaN inputs return zero variance safely.
- Session round-trip: `SessionScintillation` (de)serialises to v4 JSON
  and round-trips back to the `Scintillation` engine struct; existing
  exported preset sessions regenerated under v4.

**Hosts wired.** CLI / viewer / web.

---

### `V-25` Differential atmospheric dispersion — ⬜

**Item.** Wavelength-dependent refraction renders horizon-near point
sources as small vertical prismatic streaks (blue end higher, red end
lower) and produces the characteristic colour fringing seen on bright
low-altitude stars, planets, and the Sun / Moon limbs. The renderer
currently applies one altitude-only refraction value to all channels.

**Scientific basis.** Refraction angle `ρ(λ, h)` depends on the air
refractive index `n(λ, T, P, RH)`. Edlén 1966 gives an empirical `n(λ)`
for standard air; the differential `ρ_R − ρ_B` at altitude 5° is
≈ 1.5–2.5″, fully naked-eye visible on Sirius or the Sun's lower limb.
The "green flash" is the same effect taken to the limb of a setting Sun.

**References.**
- Filippenko, A. V. 1982, PASP 94, 715 ("The importance of atmospheric
  differential refraction in spectrophotometry").
- Stone, R. C. 1996, PASP 108, 1051 ("An accurate method for computing
  atmospheric refraction").
- Edlén, B. 1966, Metrologia 2, 71 ("The refractive index of air").
- Cox, A. N., ed. 2000, *Allen's Astrophysical Quantities*, §3.281.

**Implementation scope.**
- `crates/astronomy/src/corrections.rs`: extend
  `refraction_saemundsson` into
  `refraction_per_wavelength(altitude_rad, pressure_hpa, temperature_c, wavelength_nm)`.
  Channel representatives R=620, G=550, B=440 nm.
- `crates/renderer/src/shaders/star.wgsl`: compute three apparent
  altitudes per star and offset the screen-space position per RGB channel
  before PSF accumulation, so dispersion is baked into the PSF footprint
  rather than added as a post-process tint.
- Sun / Moon disk shader: shift R / G / B sampling of the disk by the
  same vertical offset, giving a red lower limb / blue upper limb on a
  setting Sun.

**Tests / validation.**
- Unit: at altitude 5°, 1013 hPa, 10 °C, `refraction(B) − refraction(R)`
  ∈ [1.2″, 2.5″].
- Pinned scene: Sirius at altitude 3°, screenshot showing visible R-B
  offset; sunset scene shows reddened lower limb.

**Dependencies.** Extends `V-29` (refraction) with wavelength dependence.

**Hosts wired.** CLI / viewer / web.

---

### `V-26` Lunar earthshine — ⬜

**Item.** The Moon's dark side glows faintly during crescent phases, lit
by sunlight reflected from Earth (Da Vinci glow / ashen light). Currently
the lunar disk shader lights only the sunlit fraction, so crescent phases
miss this characteristic look.

**Scientific basis.** Earthshine luminance is

```
L_dark = E_sun · α_Earth · α_Moon · f_phase(Earth-from-Moon)
```

Goode et al. 2001 calibrated the dark-side surface brightness to V ≈ +3.7
mag/arcsec² near new Moon, with strong phase dependence: when the Moon
shows a thin crescent, Earth (from the Moon) is near full, and vice versa
— so `f_phase(Earth-from-Moon) ≈ (1 − moon_illuminated_fraction)` at
first order.

**References.**
- Danjon, A. 1936, Annales de l'Observatoire de Strasbourg 3, 139
  ("Photometric measurements of earthshine").
- Goode, P. R. et al. 2001, GRL 28, 1671 ("Earthshine observations of
  the Earth's reflectance").
- Qiu, J. et al. 2003, JGR 108, D22 (phase dependence and
  Bond-albedo retrieval).

**Implementation scope.**
- `crates/astronomy/src/illuminants.rs`:
  `earthshine_disk_luminance_cd_m2(moon_phase_angle_rad, earth_albedo,
  lunar_albedo)`.
- `crates/renderer/src/shaders/skyglow.wgsl`: lunar disk fragment shader
  composes `(lit_side_lambertian + dark_side_earthshine_lambertian)`.
  Surface-brightness anchor: V = 3.7 mag/arcsec² at moon_phase_angle = 60°
  (typical crescent).
- Anchor extinction so the dark side is properly attenuated at low
  altitudes by the same per-channel extinction the rest of the scene
  uses.

**Tests / validation.**
- Unit: ratio dark / lit ≈ 10⁻³ at 5% illuminated, ≈ 0 at full moon,
  monotonic in between.
- Pinned scene: 5% crescent over a moonless dark sky preset; assert
  dark-side pixel luminance within ±0.5 mag/arcsec² of reference.

**Hosts wired.** CLI / viewer / web.

---

### `V-27` Belt of Venus and Earth-shadow band — ⬜

**Item.** Twilight is not radially symmetric around the zenith. Looking
opposite the Sun during civil twilight, observers see a pink "anti-twilight
arch" (the Belt of Venus, ~10–15° above the horizon, lit by red-attenuated
forward-scattered sunlight) directly above a darker blue-grey band (the
Earth's shadow). The current twilight model uses a zenith-only luminance
scaled by smoothstep, so this characteristic two-colour anti-solar
gradient is not reproduced.

**Scientific basis.** Hulburt 1953 first explained the Belt of Venus as
red-pass single-scattering from atmosphere already cooled by Rayleigh-
stripping of blue along the long anti-solar path. Lee & Hernández-Andrés
2003 measured the radiance and chromaticity field as a function of solar
depression and relative azimuth, providing the empirical lookup we fit.

**References.**
- Hulburt, E. O. 1953, JOSA 43, 113 ("Explanation of the Brightness and
  Color of the Sky, particularly the Twilight Sky").
- Lee, R. L. Jr., Hernández-Andrés, J. 2003, Appl. Opt. 42, 445
  ("Measuring and modeling twilight's purple light").
- Adams, C. N., Plass, G. N., Kattawar, G. W. 1974, J. Atmos. Sci. 31,
  1662.

**Implementation scope.**
- `crates/astronomy/src/atmosphere.rs`:
  `antitwilight_arch_radiance(sun_alt_rad, relative_az_rad, view_alt_rad)`
  returning per-channel radiance, and `earth_shadow_band_radiance(...)`
  for the dark band beneath.
- `crates/renderer/src/shaders/skyglow.wgsl`: extend the twilight
  composition with a `(relative_az, view_alt)` 2-axis fit. Anti-solar
  0–15° altitude: warm pink ramp (Hulburt long-path Rayleigh leftover).
  −2° to +2° altitude anti-solar: cool blue / grey Earth shadow.

**Tests / validation.**
- Unit: at sun_alt = −2°, the anti-solar (relative_az = 180°) radiance
  at view_alt = 5° has positive red excess vs. zenith reference; the
  band at view_alt = 0° has the lowest luminance in the anti-solar
  half-sky.
- Pinned scenes: `civil-twilight-antisolar-tokyo` PNG + numerical
  assertion on R / G ratio in two ROI pixels.

**Dependencies.** Extends `V-33` (twilight blend) with azimuthal
resolution.

**Hosts wired.** CLI / viewer / web.

---

### `V-28` Spectral airglow decomposition — ⬜

**Item.** Replace the single dark-sky airglow floor (currently one S10
constant feeding all channels equally) with the three dominant atmospheric
emission components: O I 557.7 nm green line, Na D 589 nm, and OH Meinel
red / IR bands. This gives the airglow its characteristic faint green /
red mottled tint and removes the unphysical pure-grey night floor.

**Scientific basis.** Leinert et al. 1998 §7.5 tabulates zenith airglow
surface brightness by component for a "moderate-activity" night: green
line ≈ 250 R, Na D ≈ 30 R, OH continuum ≈ 800 R (integrated through the
V band). The Van Rhijn function `(1 − 0.96 sin²z)^(−1/2)` gives the limb
brightening for each emitting layer (≈ 90 km for O I, ≈ 87 km for OH).

**References.**
- Leinert, Ch. et al. 1998, A&AS 127, 1, §7.4–7.6.
- Krassovsky, V. I., Shefov, N. N., Yarin, V. I. 1962, Planet. Space Sci.
  9, 883 (OH Meinel bands).
- Roach, F. E., Gordon, J. L. 1973, *The Light of the Night Sky*.

**Implementation scope.**
- `crates/astronomy/src/skyglow.rs`:
  `airglow_components(altitude_rad, activity_level) ->
  (green_557_s10, sodium_589_s10, oh_red_s10)`, plus a Van Rhijn
  layer-altitude correction per component.
- `crates/renderer/src/shaders/skyglow.wgsl`: replace the
  `AIRGLOW_FLOOR_S10` constant with per-channel sums weighted by the
  V-band response of each emission line.
- Per-channel weights: green line dominates G, OH bands dominate R, Na D
  contributes warm yellow.

**Tests / validation.**
- Unit: zenith total integrated airglow in V band within 10% of Leinert
  §7 reference.
- Pinned dark-sky scene re-render with the new decomposition; G / R
  chromaticity difference vs. neutral grey ≥ a documented threshold.

**Hosts wired.** CLI / viewer / web.

---

## Refraction and apparent place

### `V-29` Atmospheric refraction — ✅ done

**Item.** Saemundsson 1986-style apparent-altitude correction with
pressure / temperature controls, applied to stars and to Sun / Moon disk
directions. Up to 34′ at the horizon. The visible effect on the screen
(Sun visibly above the geometric horizon at sunset, stars at the horizon
lifted by ~half a Moon diameter) is the motivation, which is why refraction
sits on the Visual track even though it is mechanically a coordinate
correction.

**Reference.** Saemundsson, T. 1986, S&T 72, 70; Meeus 1998 Ch. 16.

**Implementation.** `astronomy::corrections`, `renderer::Atmosphere`,
`renderer::camera`, `shaders/star.wgsl`.

**Tests / validation.** Refraction at altitudes 90°, 45°, 10°, 5°, 0°
within tabulated Meeus values; zero at vacuum-pressure setting.

**Dependencies.** `V-25` extends this with wavelength dependence
(dispersion).

---

## Sun, Moon, planets, and sunlit atmosphere

### `V-30` Sun and Moon apparent topocentric state — ✅ done

**Item.** Direction, angular radius, illuminated fraction, and disk-
rendering inputs for the Sun and Moon, with WGS84 topocentric parallax.

**Implementation.** VSOP87 / FK5 Sun + ELP2000-style Moon from the `astro`
crate, followed by WGS84 topocentric parallax. Feeds scattering, twilight,
moon phase, and disk rendering.

**Tests / validation.** Sun apparent altitude at noon Tokyo solstice within
arcminute of Stellarium; lunar parallax shift between zenith and horizon
matches the expected ~1° at typical Earth-Moon distance.

**Dependencies.** Precision upgrade tracked in `L-06` (DE440).

---

### `V-31` Solar / lunar illuminants — ✅ done

**Item.** Spectral or XYZ irradiance for direct sunlight and moonlight at
the top of the atmosphere; downstream daylight and moonlit shading consume
these as physical illuminants rather than hard-coded colours.

**References.** CIE daylight-basis / ASTM G-173 solar XYZ irradiance;
Krisciunas, K., Schaefer, B. E. 1991, PASP 103, 1033 (lunar phase
photometry).

**Implementation.** `astronomy::illuminants`.

**Tests / validation.** Solar illuminance at top-of-atmosphere ≈ 128 000
lux; full Moon at zenith ≈ 0.25 lux.

---

### `V-32` Sunlit atmospheric scattering — ✅ done

**Item.** Rayleigh + Mie aerosol + ozone absorption sky model driven by
Sun altitude, view direction, observer altitude, and turbidity. Produces
blue daylight, golden-hour warmth, sunset reddening, and horizon haze.

**Reference.** Preetham, A. J., Shirley, P., Smits, B. 1999, SIGGRAPH '99
(Perez / CIE-basis daylight model).

**Implementation.** `astronomy::atmosphere`,
`crates/renderer/src/shaders/skyglow.wgsl`.

**Tests / validation.** Daylight-domain Rust luminance tests pinned at
noon, sunset, civil twilight; Preetham's documented validity window is
the model boundary.

**Dependencies.** Superseded by `V-38` (Hošek-Wilkie 2012); the
Preetham evaluator and turbidity helpers have been removed from the
renderer.

---

### `V-33` Twilight and day / night blend — ✅ done

**Item.** Combine sunlit scattering, moonlit sky, dark-sky glow, and star
visibility using solar depression angle rather than hard-coded background
colours.

**Implementation.** `astronomy::atmosphere`, `astronomy::skyglow`,
`shaders/skyglow.wgsl`. Solar-depression twilight radiance is continuous
across civil / nautical / astronomical bands and composed additively
with moonlit sky and the dark-sky terms.

**Tests / validation.** Rust tests pin model-domain boundaries; gallery
covers all four solar-depression regimes.

**Dependencies.** `V-27` extends this with azimuthally-resolved Belt of
Venus and Earth-shadow band.

---

### `V-34` Atmosphere controls — ✅ done

**Item.** Turbidity, aerosol, observer altitude, and optional ozone /
visibility presets exposed in CLI, viewer, and web settings, with defaults
matching a clear rural sky and full serialisation through JSON sessions.

**Implementation.** `apps/{cli,viewer,web}`, `crates/common`.

**Tests / validation.** Session roundtrip covers every atmosphere control;
preset list documented in `docs/scene-presets.md`.

**Dependencies.** `V-37` introduces (β, α, DU) controls that replace the
single turbidity scalar; existing sessions auto-migrate.

---

### `V-35` Planets — ✅ done

**Item.** Mercury → Neptune apparent positions, magnitudes, and disk /
point rendering in the skyglow pass.

**Implementation.** VSOP87D light-time-corrected apparent states from
`astro`, topocentric parallax, and renderer planet rendering.

**Tests / validation.** Cross-check against Stellarium / Horizons at fixed
epochs; documented ~1″ on a century budget.

**Dependencies.** Precision upgrade tracked in `L-06` (DE440).

---

### `V-36` Moon phase + Earth-shadow aid — ✅ done

**Item.** Moon disk phase rendered from the apparent Sun-Moon-Earth
geometry; lunar-eclipse umbral contact exposed as a darkening aid.

**Implementation.** `astronomy::ephemeris`, `shaders/skyglow.wgsl`.

**Tests / validation.** Phase angle at known new / full epochs; umbral
penumbra geometry verified against documented eclipse circumstances.

---

### `V-37` Unified spectral extinction model — ✅ done

**Item.** Replaced the per-channel Hardie / Schaefer extinction
coefficients (three loose constants) and the independent Preetham
turbidity input with one extinction model

```
k(λ) = k_Rayleigh(λ) + k_Aerosol(λ, β, α) + k_Ozone(λ, DU)
```

evaluated at R / G / B representative wavelengths and consumed by both
the star-extinction path and the daylight scattering shader. Today the
two systems can disagree about how reddened a given sky should be.

**Scientific basis.** Schaefer 1993 §3 derives `k(λ)` as the
Rayleigh + Aerosol + Ozone sum for sea-level standard atmospheres. The
Ångström turbidity formula `k_a(λ) = β · (λ/550)^(−α)` (Ångström 1929)
replaces a single turbidity scalar with the physically meaningful β
(aerosol optical depth at 550 nm) and α (size-distribution power). Ozone
Chappuis absorption peaks near 600 nm; column DU values 200–500 cover
global variability.

**References.**
- Schaefer, B. E. 1993, Vistas in Astronomy 36, 311, §3.
- Ångström, A. 1929, Geografiska Annaler 11, 156.
- Hayes, D. S., Latham, D. W. 1975, ApJ 197, 593 (Mauna Kea / Mt. Hopkins
  extinction with k_Rayleigh + k_a + k_O3 separation).
- Iqbal, M. 1983, *An Introduction to Solar Radiation*, §6.5.

**Implementation.**
- `crates/astronomy/src/atmosphere.rs`:
  `extinction_coefficients(wavelength_nm, h_obs_m, beta, alpha, ozone_du)
  -> ExtinctionTerms { rayleigh, aerosol, ozone }` plus `extinction_k_rgb`
  evaluated at the same R / G / B representative wavelengths the catalogue
  blackbody pipeline uses, and `preetham_turbidity_from_aerosol(β)` as the
  bridge to the daylight shader.
- `crates/astronomy/src/photometry.rs`: removed the legacy
  `DEFAULT_EXTINCTION_K_RGB` constant; `extinction_magnitudes_rgb` now
  consumes the unified-model coefficients.
- `crates/renderer/src/camera.rs`: `Atmosphere` keeps only the canonical
  `(aerosol_beta, aerosol_alpha, observer_altitude_m, ozone_du, pressure,
  temperature)` state. `extinction_k_rgb()` and the daylight uniform's
  effective turbidity are derived on the fly so the two paths cannot drift.
- `crates/renderer/src/shaders/skyglow.wgsl`: daylight haze whitening and
  twilight aerosol load both read `β` from the shared uniform; the legacy
  `visibility_km` input is gone.
- `crates/common`: `Atmosphere`, `AtmosphereOverrides`, `SessionAtmosphere`,
  and the CLI / viewer / web hosts all expose `(β, α, DU)` directly.
  Session schema bumped to v2; the legacy `turbidity` / `visibilityKm`
  fields are removed.

**Tests / validation.**
- Unit: at sea level with (β=0.10, α=1.3, DU=300), `k_V` matches Hardie
  1962 mid-quality site within 0.03 mag/airmass; monotonicity in β / α /
  DU; Rayleigh and aerosol thin together with the 8 km scale height while
  ozone (stratospheric) is untouched.
- Renderer uniform regression: NaN host values fall back to the DEFAULT
  (β, α, DU, h) state and produce the same derived `k_RGB` and
  Preetham-effective turbidity as the default scene.

**Dependencies.** Replaces the loose coupling between `V-18` (extinction)
and `V-32` (daylight scattering). The `V-38` Hošek-Wilkie upgrade will
reuse the same `(β, α, DU)` state without further schema churn.

**Hosts wired.** CLI / viewer / web.

---

### `V-38` Hošek-Wilkie daylight sky model — ✅ done

**Item.** Replace Preetham (`V-32`) with the Hošek & Wilkie 2012 sky dome
model as the default daylight radiance source. Preetham 1999 breaks down
at solar altitude < 5° (its asymptotic Perez fit goes negative or
unphysical) and produces wrong colours at high turbidity — exactly the
configurations the renderer cares about for sunrise / sunset.
Hošek-Wilkie was published as the replacement and is used by every modern
offline renderer.

**Scientific basis.** Hošek-Wilkie expands radiance as a 9-parameter
polynomial in `cos θ` and `γ` with coefficient tables fit to brute-force
Mishchenko spectral RT solutions, and remains physical down to
sun_alt = −5°. The 2012 paper covers RGB; Wilkie et al. 2021 extends it to
a spectral sky dome covering 320–720 nm in 40 bins, which would later let
us share spectra with `V-37`.

**References.**
- Hošek, L., Wilkie, A. 2012, ACM TOG 31(4), "An Analytic Model for Full
  Spectral Sky-Dome Radiance".
- Wilkie, A. et al. 2021, EGSR, "A Fitted Radiance and Attenuation Model
  for Realistic Atmospheres".
- Bruneton, E. 2017, Comp. Graph. Forum 36(2), 167 (qualitative
  comparison of analytic sky models).

**Implementation.**
- Vendored HW2012 RGB coefficient table (BSD 3-clause, upstream release
  v1.4a, 22 Feb 2013) at
  `crates/astronomy/data/hosek_wilkie/coefficients_rgb.bin`, packed by
  `scripts/build-hosek-wilkie.py` and pinned in `data/manifest.toml`
  under `hosek-wilkie-2012-rgb-v1.4a`.
- `crates/astronomy/src/atmosphere/hosek_wilkie.rs`: parser + `cook(t,
  albedo, sun_elev) -> HosekWilkieParams` (quintic-Bezier blend across
  the elevation control points, linear interpolation in turbidity and
  ground albedo) + `radiance(params, θ, γ) -> [f64; 3]`.
- `crates/renderer/src/camera.rs`: per-frame CPU `cook` call; nine
  vec4 coefficient rows + a radiance vec4 added to `CameraUniform`.
- `crates/renderer/src/shaders/skyglow.wgsl`:
  `hosek_wilkie_sky_luminance_rgb` evaluator replaces the Preetham
  daylight path entirely. The legacy Preetham WGSL evaluator and the
  Rust `preetham_zenith_luminance_cd_m2` helper are removed; the
  shared Linke-turbidity bridge stays under the model-neutral name
  `linke_turbidity_from_aerosol` in `astronomy::atmosphere`.
- `crates/renderer/src/camera.rs` adds `Atmosphere::surface_albedo`;
  CLI / viewer / web all expose `--surface-albedo` (plus matching web
  UI control). `SessionAtmosphere` adds `surfaceAlbedo` as a required
  field; session schema bumps to v3 since the daylight model is no
  longer a stored choice.

**Tests / validation.**
- Unit: HW radiance is finite and non-negative across the upper
  hemisphere at sun_alt = 1°, T = 4 (Preetham asymptote fails this).
- Unit: zenith luminance monotone in turbidity and ground albedo;
  per-channel zenith ordering satisfies B > G > R for clear sky.
- Unit: HW zenith luminance at the noon reference (T = 2.5, albedo =
  0.10, sun_alt = 60°) lies in the published 1–15 kcd/m² daylight
  range after the 683 lm/W radiometric → photometric conversion.
- Unit: cooked configuration returns the zero sentinel below the
  horizon so the shader stays branch-free.

**Hosts wired.** CLI / viewer / web.

---

### `V-39` Light pollution / Bortle map — ⬜

**Item.** The current dark-sky pipeline assumes a clear rural site
(V ≈ 21.6 mag/arcsec² zenith). Real observers want the sky they will
actually see from Tokyo, downtown LA, or a national park. Add an
observer-side `LightPollution` config that scales the dark-sky background
by either (a) a Bortle 1–9 selector, (b) a manual SQM mag/arcsec² value,
or (c) a sample from Falchi et al. 2016's World Atlas loaded by lat / lng.

**Scientific basis.** Bortle 2001 defines nine site classes by zenith sky
brightness (Class 1: V ≈ 21.99; Class 9: V ≈ 16.5). Falchi et al. 2016
published a global VIIRS-derived zenith artificial-brightness atlas at
~750 m resolution under a documented licence. Cinzano, Falchi, Elvidge
2001 give the long-form scattering model from which both follow.
Artificial-sky-glow spectrum is sodium / LED-dominated (warm orange to
mixed), not neutral.

**References.**
- Bortle, J. E. 2001, S&T 101(2), 126 ("Introducing the Bortle Dark-Sky
  Scale").
- Falchi, F. et al. 2016, Science Advances 2, e1600377 ("The new world
  atlas of artificial night sky brightness"); accompanying global
  GeoTIFF (licence noted in `data/manifest.toml`).
- Cinzano, P., Falchi, F., Elvidge, C. D. 2001, MNRAS 328, 689.
- Garstang, R. H. 1986, PASP 98, 364 (single-scattering origin model).

**Implementation scope.**
- `crates/common`: `LightPollution { Bortle(u8), Sqm(f32),
  Atlas2016 { ... } }` added to the session schema.
- `crates/astronomy/src/skyglow.rs`: artificial-sky-glow term in S10
  units, scaled with zenith distance via Garstang's single-scattering
  kernel; spectral tint towards sodium / LED (warm orange) rather than
  neutral grey.
- `data/manifest.toml`: optional Falchi 2016 GeoTIFF entry; shipping the
  full atlas is too large to commit, so a downloader script under
  `scripts/` is the supported path. The fields used and licence caveat
  are documented per the manifest rules.
- `crates/renderer/src/shaders/skyglow.wgsl`: artificial component added
  to the night-sky composition before extinction.

**Tests / validation.**
- Unit: zenith brightness for Bortle 5 ≈ V 20.0 within 0.2 mag.
- Scenes: `tokyo-bortle-8`, `dark-sky-bortle-1` pinned screenshots in
  the validation gallery.

**Hosts wired.** CLI / viewer / web.

---

## Projections, viewpoints, and instruments

### `V-40` Full-sky projections — ✅ done

**Item.** Mollweide, Aitoff, and Hammer all-sky maps selectable in CLI,
desktop, and web; perspective remains the default.

**Scientific basis.** Standard equal-area map projections; geometry from
Snyder 1987, USGS PP 1395.

**Implementation.** `renderer::SkyProjection`,
`shaders/{star,skyglow,overlay}.wgsl`, `apps/{cli,viewer,web}`.

**Tests / validation.** Each projection has a pinned scene and a forward
/ inverse roundtrip test.

---

### `V-41` Out-of-Earth galactic-north viewpoint — ✅ done

**Item.** `SkyViewpoint::GalacticNorth` moves the camera above the IAU
galactic plane, places HYG stars by parsec distance, and draws an
analytic top-down Milky Way disc.

**Implementation.** `renderer::SkyViewpoint`,
`shaders/{star,skyglow}.wgsl`, `apps/{cli,viewer,web}`.

**Tests / validation.** Pinned scene + parsec-coordinate roundtrip tests.

**Dependencies.** `V-46` upgrades the analytic disc to a
Drimmel-Spergel + Reid arm-trace model.

---

### `V-42` Deep-sky overlay — ✅ done (Messier + bright NGC / IC subset)

**Item.** Messier objects, plus a bright NGC / IC subset embedded
alongside them, with density controls. Each object renders as a small
extended marker (size from catalog dimensions) plus a label.

**Scientific basis.** Messier 1781 (110 objects, well-documented historic
catalogue); NGC / IC (Dreyer 1888 / 1908, modernised in OpenNGC under a
permissive licence). Object dimensions follow the catalogue's published
major / minor-axis fields.

**References.**
- Messier, C. 1781, *Catalogue des Nébuleuses et des Amas d'Étoiles*.
- Dreyer, J. L. E. 1888, MmRAS 49, 1 (NGC); 1908, MmRAS 59, 105 (IC).
- Frommert, H., Kronberg, C. (current), *SEDS Messier Database*.
- Verga, M. (current), OpenNGC — maintained NGC / IC compilation.

**Status — Messier slice (PR #48).**
- 110 Messier objects rendered as diamond markers with `M1`..`M110` text
  labels, gated by a V-magnitude density slider exposed in CLI / desktop /
  web. Default magnitude limit 7.0 reveals the canonical naked-eye
  showpieces; slider up to 99 shows every Messier object.

**Status — NGC / IC subset (this PR).**
- Trait abstraction: `catalog::deepsky::DeepSkyCatalog` with embedded
  `MessierCatalog` and `NgcBrightCatalog` implementations. The renderer
  consumes the trait so the planned runtime full-OpenNGC backend slots
  in without further renderer churn.
- Embedded data: `crates/catalog/data/openngc_bright.csv` (~1,250
  objects, V ≤ 11.5 mag plus large diffuse nebulae lacking integrated
  photometry, sentinel-magnitude tagged). Produced deterministically by
  `scripts/extract-openngc-bright.py` from the upstream OpenNGC snapshot.
- Marker shape differentiation: Messier objects keep their 4-segment
  diamond; NGC / IC objects render as an 8-segment ring so the user reads
  the catalogue at a glance without consulting the label.
- Label tinting: Messier labels stay warm green; NGC / IC labels are a
  slightly cooler teal so the marker / label pairing remains visible.
- Density control: existing `OverlayConfig::deep_sky_magnitude_limit`
  slider applies to both catalogues simultaneously — no new host
  plumbing required.
- Provenance: relocated `messier-catalog` row in `data/manifest.toml`
  (now under `crates/catalog/data/`) plus the new `openngc-bright-catalog`
  row with the extraction script as `preprocessing`.

**Tests / validation.**
- `catalog::deepsky` unit tests pin Messier completeness, anchor objects
  (NGC 7000, 253, 869 / 884, 5128, 1499, 6960, IC 434), the sentinel-
  magnitude filter policy, and inclusive-magnitude filtering.
- `renderer::overlay` updated tests pin the Messier diamond contribution
  and a multiple-of-16 NGC ring contribution at the show-all magnitude
  limit, NaN-safe gating, and slider monotonicity.
- `renderer::text` asserts the deep-sky label table contains exactly 110
  Messier entries plus the canonical NGC 7000 and IC 434 anchors.
- `stars-manifest` integration tests re-verify both deep-sky CSV hashes
  against the on-disk bytes.

**Hosts wired.** CLI / viewer / web (the existing density slider drives
both Messier and NGC / IC overlays).

**Follow-up.** A runtime streaming backend (`OpenNgcCsvCatalog`) for the
full ~14,000-entry OpenNGC catalogue is the next PR. It will surface the
Dup-marked objects and the famous diffuse nebulae with undersized
upstream `MajAx` (NGC 2244 Rosette cluster, IC 1396, IC 2118 Witch Head)
that the committed bright subset deliberately misses. Identifier
preservation through the renderer (hover / click → NGC / PGC IDs) tracks
separately as `L-18`.

---

### `V-43` Telescope eyepiece simulation — ✅ done

**Item.** Plate scale, magnification, exit pupil, and true field of view
from an OTA + eyepiece pair, exposed in CLI / desktop / web session URLs.

**Implementation.** `renderer::EyepieceSimulation`, `apps/{cli,viewer,web}`.

**Tests / validation.** Magnification / true-FOV calculation tests; scene
preset includes an eyepiece scene.

**Dependencies.** `V-45` extends this with diffraction / spider / chromatic
artifacts so the eyepiece view is recognisably different across OTA
classes.

---

### `V-44` Custom external viewpoint origin — ✅ done

**Item.** `SkyViewpoint::CustomExternal` uses user-selectable `origin_pc`,
target, and up vectors in IAU galactic Cartesian parsecs (Sun origin,
+X l=0°, +Y l=90°, +Z north galactic pole), exposed in session URLs.

**Implementation.** `renderer::ExternalViewpoint`, `apps/{cli,viewer,web}`.

**Tests / validation.** Coordinate-roundtrip tests; pinned external-view
scene.

**Dependencies.** `V-46` upgrades the rendered Galaxy from an analytic
disc to the Drimmel-Spergel + Reid arm-trace model.

---

### `V-45` Telescope-side optical artifacts — ⬜

**Item.** Extend the eyepiece simulation (`V-43`) from a geometric
magnification / FOV calculator into an optical model that reproduces the
things observers actually see in an eyepiece: Airy diffraction disc on
bright stars, diffraction spikes from spider vanes, residual chromatic
aberration in achromatic refractors, and field curvature / vignetting at
the field stop. Without this, the eyepiece mode currently looks like the
naked-eye pipeline at a higher zoom.

**Scientific basis.** Airy radius is `1.22 λ/D` from Fraunhofer
diffraction (Born & Wolf §8.5). Spider-vane diffraction is the Fourier
transform of the obscuring line, producing 2n-armed spikes for n vanes.
Achromat secondary spectrum follows Conrady's formula and is
wavelength-dependent. Field curvature / vignetting are standard
exit-pupil geometry.

**References.**
- Born, M., Wolf, E. 1999, *Principles of Optics*, 7th ed., §8.5.
- Conrady, A. E. 1929, *Applied Optics and Optical Design*, Vol. 1.
- Suiter, H. R. 2008, *Star Testing Astronomical Telescopes*, 2nd ed.
- Rutten, H. G. J., van Venrooij, M. A. M. 1988, *Telescope Optics*.

**Implementation scope.**
- `crates/renderer/src/eyepiece.rs`: `OpticalDesign { Refractor {
  achromat: bool, focal_ratio }, Newtonian { spider_vanes: u8 },
  SchmidtCassegrain { obstruction_pct } }`.
- `crates/renderer/src/shaders/star.wgsl`: bright-star PSF becomes
  Spencer (eye) ⊗ instrument (aperture-diffraction + obstruction +
  spider) convolution evaluated analytically for representative
  wavelengths.
- Add spike-orientation control (so a Newtonian's spikes rotate with
  the OTA), focal-length-driven Airy radius, and exit-pupil-relative
  vignette.

**Tests / validation.**
- Unit: Airy radius at D=200 mm, λ=550 nm is 0.69″ within 1%.
- Visual: render Vega at 200× through (a) a refractor, (b) a 4-vane
  Newtonian, (c) an SCT; pinned PNGs in the validation gallery.

**Hosts wired.** CLI / viewer / web (when eyepiece mode is active).

---

### `V-46` Galactic structural model for external viewpoints — ⬜

**Item.** The current external galactic viewpoint (`V-41`, `V-44`) draws
an analytic thin Milky Way disc. Replace this with a Drimmel & Spergel-
style multi-component model (thin disk, thick disk, central bar, four
spiral arms with Reid 2019 trace, plus a dust-extinction
Schlegel + Drimmel screen) so the external view is itself a defensible
educational graphic of the Galaxy's structure rather than a generic
ellipse.

**Scientific basis.** Drimmel & Spergel 2001 fit COBE / DIRBE 240 μm +
2MASS K-band maps to a three-component stellar + two-component dust
model. Reid et al. 2019 give arm-tracer maser parallaxes that pin arm
pitch angles and pattern locations. Robitaille et al. 2017 (mwdust) wraps
a usable Python implementation.

**References.**
- Drimmel, R., Spergel, D. N. 2001, ApJ 556, 181 ("Three-Dimensional
  Structure of the Milky Way Disk").
- Reid, M. J. et al. 2019, ApJ 885, 131 (BeSSeL VLBI maser parallaxes:
  Local, Sagittarius-Carina, Scutum-Centaurus, Perseus, Outer arms).
- Robitaille, T. P. 2017, A&A 600, A11 (mwdust).
- Bland-Hawthorn, J., Gerhard, O. 2016, ARA&A 54, 529 (review).

**Implementation scope.**
- `crates/astronomy/src/galaxy.rs`: closed-form
  `milky_way_luminosity_density(x_pc, y_pc, z_pc)` and
  `dust_extinction_az(distance_pc, l_rad, b_rad)`.
- `crates/renderer/src/shaders/skyglow.wgsl` external-viewpoint branch:
  ray-march the new functions instead of the analytic disc.
- Optional: Gaia DR3 OB stars overplotted as bright points on the arms
  (uses `L-17` catalog ingest infrastructure).

**Tests / validation.**
- Unit: solar position at (8.122 kpc, 0, 0.020 kpc) IAU 2018 returns
  local stellar density ≈ 0.1 M_sun pc⁻³.
- Visual: external view from (0, 0, 1 kpc) above the Sun should show
  Sagittarius-Carina and Perseus arms in correct azimuth.

**Hosts wired.** CLI / viewer / web.

---

## Rare and transient phenomena

### `V-47` Meteor shower display — ⬜

**Item.** Stochastic meteor rendering, anchored to the IMO Working List
of Visual Meteor Showers (radiant α / δ, peak date, ZHR, population index
r, atmospheric velocity v_∞). On-screen meteors appear from the radiant
at the configured rate, with deterministic seeding so the same JSON
session reproduces the same meteor stream.

**Scientific basis.** Koschack & Rendtel 1990 give the visual ZHR
formalism: observed rate is `ZHR · sin(h_R) · r^(6.5 − lim_mag) /
F_correction`. Per-meteor magnitude follows the population index `r`.
Trail geometry from velocity vector × atmospheric entry geometry.

**References.**
- Koschack, R., Rendtel, J. 1990, WGN 18, 44 (visual flux model).
- Rendtel, J. et al. (annual), *IMO Meteor Shower Calendar*.
- McKinley, D. W. R. 1961, *Meteor Science and Engineering*.

**Implementation scope.**
- `crates/astronomy/src/meteors.rs`: shower catalog struct, expected
  observed rate at observer time / location, deterministic Poisson stream
  from a session seed.
- `crates/renderer/src/shaders/meteor.wgsl`: streak rendering with
  magnitude → length / brightness mapping; one-frame appearance, no
  persistent train.
- `data/manifest.toml`: IMO Working List ingested (transcribed constants
  from peer-reviewed papers if the live download is not permissively
  licensed).

**Tests / validation.**
- Unit: Perseid ZHR=100 at radiant alt 60°, lim mag 6.0 →
  ≈ 100 m/h ±10%.
- Visual: deterministic seed → identical meteor stream across hosts.

**Deliberate non-goal scope.** Persistent trains, fireball flares, and
meteoroid-fragmentation physics are out of scope. The renderer reproduces
the *statistical expectation* of a shower, not individual fireball events.

**Hosts wired.** CLI / viewer / web.

---

### `V-48` Aurora display — ⬜

**Item.** Optional high-latitude aurora overlay driven by a Kp / Hp index
input (offline-supplied via the JSON session or fetched from NOAA SWPC
when online and authorised). Aurora oval geometry follows Feldstein 1963
/ Holzworth & Meng 1975. Visually: green O I 557.7 nm arc + an occasional
red O I 630.0 nm upper-altitude band + magenta N₂ lower borders.

**Scientific basis.** Feldstein & Starkov 1967 maps the auroral oval
boundary in corrected geomagnetic latitude as a function of Q index.
Emission heights and colours: Chamberlain 1961, *Physics of the Aurora
and Airglow*. OVATION Prime (Newell et al. 2010) is the modern stochastic
auroral oval model.

**References.**
- Feldstein, Y. I., Starkov, G. V. 1967, Planet. Space Sci. 15, 209.
- Akasofu, S.-I. 1964, Planet. Space Sci. 12, 273 (substorm phases).
- Chamberlain, J. W. 1961, *Physics of the Aurora and Airglow*.
- Newell, P. T. et al. 2010, JGR 115, A03216 (OVATION Prime).

**Implementation scope.**
- `crates/astronomy/src/aurora.rs`:
  `auroral_oval_boundary(kp, season) ->
  (geomag_lat_equatorward, geomag_lat_poleward)`.
- `crates/renderer/src/shaders/skyglow.wgsl`: optional auroral emission
  layer composited above the dark-sky base. Vertical structure:
  100–120 km O I green dominant, 200–250 km O I red, < 100 km N₂ magenta
  border.
- Geomagnetic-coordinate transform (corrected geomagnetic, IAGA AACGM
  reference epoch) added to `astronomy::corrections`.

**Tests / validation.**
- Unit: at Kp=4, equatorward boundary at corrected geomagnetic latitude
  ≈ 63° within 1°.
- Visual: Tromsø scene at Kp=5 shows green arc in the northern sky.

**Deliberate non-goal scope.** Real-time aurora morphology (curtains,
rays, dynamic substorm motion) is intentionally not modelled. The
renderer shows the *statistically expected* oval position and brightness
for the supplied Kp.

**Hosts wired.** CLI / viewer / web.

---

### `V-49` Comet rendering — ⬜

**Item.** Render named comets from a documented orbital-element snapshot
(JPL Small-Body Database / Minor Planet Center MPCORB) with both coma and
dust / ion tail. Position from two-body Keplerian propagation from
osculating elements at epoch (extended to N-body when DE440 lands via
`L-06`).

**Scientific basis.** Comet orbits use Marsden conventions in the MPC
element format. Coma photometry follows the standard
`m1 = M1 + 5 log Δ + 2.5 n log r` (Bobrovnikoff-Bowell magnitude law).
Dust tail direction follows Finson-Probstein 1968 syndyne / synchrone
families for `β = F_rad / F_grav`; ion tail follows the antisolar
direction projected with solar-wind aberration.

**References.**
- Finson, M. L., Probstein, R. F. 1968, ApJ 154, 327 (Part I: dust tail
  dynamics).
- Marsden, B. G., Williams, G. V. (current), MPC element format
  documentation.
- Bobrovnikoff, N. T. 1942, ApJ 95, 71; Bowell, E. et al. 1989 (comet
  magnitude law conventions).
- Belton, M. J. S. et al. 2018, *Comets II / III* relevant chapters.

**Implementation scope.**
- `crates/astronomy/src/comets.rs`: osculating-element struct, Keplerian
  propagation to date, magnitude law.
- `crates/renderer/src/shaders/skyglow.wgsl`: coma rendered as a soft
  circular profile with surface brightness ∝ `1/ρ`; dust tail rendered
  along the syndyne for representative `β = 0.6`, ion tail along the
  antisolar projection.
- `data/manifest.toml`: documented snapshot of selected named comets
  (Halley, Hale-Bopp historic; C/2023 A3 and current bright comets) with
  epoch and source DOI.

**Tests / validation.**
- Unit: Halley's comet position 1986-04-10 within 1′ of Marsden's
  published value.
- Visual: deterministic comet renders for a fixed set of bright-comet
  scenes.

**Deliberate non-goal scope.** No fragmentation, no outburst modelling,
no jet structure. Single-epoch element sets are regenerated when the
manifest version bumps.

**Hosts wired.** CLI / viewer / web.

---

## Output colour

### `V-50` Output colour management — ⬜

**Item.** Today the renderer writes sRGB-encoded RGB and
`docs/standards-compliance.md` notes that no colour management is
performed. Add explicit output-space selection (sRGB / Display-P3 /
Rec.2020) and embed the chosen primaries in session JSON, so PNG renders
sent between hosts produce the same colour on a calibrated screen.

**Scientific basis.** IEC 61966-2-1 (sRGB), SMPTE EG 432-1 / Apple
Display-P3 (P3 D65), ITU-R BT.2020-2 (Rec.2020). Gamut transform is a
3×3 matrix between linear primaries; tone curve depends on the chosen
space. The Reinhard tonemap already runs in linear radiance, so colour
management is a post-gamut matrix + transfer-function swap.

**References.**
- IEC 61966-2-1:1999 (sRGB).
- SMPTE EG 432-1:2010 (DCI-P3 family; Display-P3 via EOTF substitution).
- ITU-R BT.2020-2 (Rec.2020).

**Implementation scope.**
- `crates/renderer/src/tonemap.rs` + `tonemap.wgsl`: final 3×3 + transfer
  function selectable via uniform.
- `crates/common`: session schema field `output_colourspace`.
- `apps/cli`: `--output-colourspace {srgb,display-p3,rec2020}`; PNG
  cICP / sRGB / P3 chunk written by the encoder.
- `apps/web`: WebGPU swap-chain colourspace selection where supported,
  with documented fallback to sRGB on unsupported browsers.

**Tests / validation.**
- Unit: round-trip a pure red primary through the chosen gamut matrix
  and check chromaticity.
- Visual: same scene rendered to sRGB and P3 should produce different
  but predictable chromaticity logs.

**Hosts wired.** CLI / viewer / web.

---

## Solar system geometry — eclipses and occultations

### `V-51` Unified eclipse / occultation pass — ⏳ (`V-51a` + `V-51b` + `V-51c` shipped)

**Item.** Today only the lunar eclipse is modelled (`V-36`: Earth's umbra
darkening the Moon). Every other foreground/background pair where one
solar-system body hides another is missing: solar eclipses, lunar
occultations of stars and planets, Mercury / Venus transits across the
Sun, and the (rare) mutual planetary occultations. Add one common
geometry + render path that handles all of them from a given observer,
with deterministic visual support and contact-timing checks against
published circumstances.

**Performance contract.** All occlusions are sub-degree screen regions,
so the path is **analytic angular masks**, not depth / stencil. The
hot skyglow fragment shader gains one `subtract` branch per occluder
(`N ≤ ~10`); star sprites are culled CPU-side by angular separation
against occluders (`10⁴ × 10 ≈ 0.1 ms`); the corona is drawn only at
totality, inside a 2° scissor rect. No GPU depth buffer, no full-screen
re-passes. Target: no measurable fps regression on the existing
benchmark scenes.

**Scientific basis.**
- Apparent positions come from existing `apparent_*_topocentric`
  helpers (VSOP87D + ELP2000-style + WGS84 parallax). Pair-wise
  classification by angular separation `Δ` vs apparent radii
  `r_front`, `r_back`:
  - **none** when `Δ ≥ r_front + r_back`,
  - **partial** when `|r_front − r_back| < Δ < r_front + r_back`,
  - **annular / transit** when `Δ ≤ r_back − r_front` and `r_front < r_back`,
  - **total / full occultation** when `Δ ≤ r_front − r_back`.
- Sky brightness during solar totality reduced via Koomen et al. 1952
  illuminance falloff applied to the diffuse-sky source term — no
  Mie multiple-scattering re-solve.
- Corona: Baumbach 1937 `B(r) = a r^−2.5 + b r^−7 + c r^−17`,
  normalized so the inner corona matches ~10^−6 of mean solar disk
  brightness; rendered only when `kind == Total` for the
  Moon → Sun pair.

**References.**
- Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 54 (solar
  eclipse circumstances; Besselian elements).
- Espenak, F., Meeus, J. 2006, NASA TP-2006-214141, *Five Millennium
  Canon of Solar Eclipses* (validation circumstances).
- Espenak, F., Meeus, J. 2009, NASA TP-2009-214174, *Five Millennium
  Canon of Lunar Eclipses*.
- Baumbach, S. 1937, Astronomische Nachrichten 263, 121 (coronal
  brightness law).
- Koomen, M. J. et al. 1952, J. Opt. Soc. Am. 42, 353 (sky brightness
  during total solar eclipse).
- IOTA (International Occultation Timing Association) predictions for
  lunar occultation contact timing validation.

**Sub-items.**

- **`V-51a` Common occlusion primitives.** ✅ Shipped.
  `crates/astronomy/src/occultation.rs` exposes `ApparentDisk`,
  `classify_disks(front, back) -> OccultationKind { None, Partial,
  AnnularOrTransit, Total }`, `obscuration_fraction(front, back) -> f32`
  (closed-form two-circle lens area / back area), and
  `contact_times(start, end, disks) -> ContactTimes` returning the
  canonical P1..P4 instants via a 30 s grid scan + bisection refine.
  Pure geometry, no rendering. The V-36 Earth-shadow aid in
  `apparent_moon` now delegates to `obscuration_fraction` so the
  lunar-eclipse and solar-eclipse paths share one definition.
- **`V-51b` Renderer analytic-mask path.** ✅ Shipped. The single-pair
  Moon-on-Sun subtract was generalised into a `MAX_OCCLUDERS = 16`
  uniform array — two `vec4` rows per entry, packed by
  `CameraUniform::occluders` and counted by
  `CameraUniform::occluder_params.x`. The producer side lives in
  `astronomy::active_occluders(observer)` (V-51c populates only the
  Moon-on-Sun pair; V-51d/e/f will plug their own front-disk pairs
  into the same list). Front-disk directions are run through the same
  `apparent_disk_direction_j2000` pipeline as the Sun and Moon
  uniforms so the analytic mask stays bit-identical to V-51c when only
  one pair is active — the committed
  `docs/assets/validation/solar-eclipse.png` golden frame is the
  contract. `shaders/skyglow.wgsl::occluder_subtract_mask(ray_dir,
  target_code, pixel_sr)` is the shared union-of-disks helper consumed
  by both the Sun and Moon disk source terms. No depth / stencil
  attachments added. The dormant CPU star-sprite cull tracked by
  [`OccluderTarget::Stars`](crates/astronomy/src/occultation.rs)
  ships with `V-51d` when its producer wires up.
- **`V-51c` Solar eclipse (Moon → Sun).** ✅ Shipped. Wires V-51a to
  the Sun ↔ Moon pair via `solar_eclipse_state(observer)` and the
  GPU `CameraUniform::solar_eclipse_state` quad-vector
  `[kind_code, obscuration, totality_weight, partial_weight]`.
  `shaders/skyglow.wgsl` extends `sun_moon_disk_radiance` with the
  analytic Moon-on-Sun subtract plus an inline Baumbach 1937 corona
  term (2° scissor, totality-only); the Hošek-Wilkie daylight branch
  and the twilight branch both multiply through a Koomen 1952
  falloff that drops to ~1e-4 of normal daylight at maximum
  obscuration. Bailey's beads / diamond ring emerge from the
  existing HDR + glare path against the analytic mask. New
  `SolarEclipse` deterministic preset (2024-04-08 Mazatlán
  totality, az≈138°, alt≈70°, 5° FoV) replaces the lunar-only
  `EclipseAid` as the validation point for the solar pipeline.
  `find_solar_eclipse(observer, start, end)` exposes peak +
  contacts to planning hosts.
- **`V-51d` Lunar occultation of stars and planets (Moon → star/planet).**
  CPU-side cull of HYG catalog sprites and planet sprites whose
  topocentric direction lies inside the Moon's disk. Disappearance /
  reappearance contact times exposed via `planning.rs`.
- **`V-51e` Mercury / Venus transit of the Sun (planet → Sun).** Planet
  apparent disk drawn as a black sub-circle inside the Sun sprite via
  the same subtract path; partial / interior contact times exposed.
- **`V-51f` Mutual planetary occultation (planet → planet).** Same
  classify + mask path applied to the planet ↔ planet pairs. Rare in
  practice; validated only at known historical events (e.g. Venus
  occults Jupiter 1818-01-03).

**Implementation scope.**
- `crates/astronomy/src/occultation.rs` (new): pair-wise classifier,
  obscuration, contact-time helpers.
- `crates/astronomy/src/ephemeris.rs`: reuse for `V-36` lunar-eclipse
  fraction (the existing geometry collapses into the new helpers).
- `crates/astronomy/src/planning.rs`: `eclipse_events(window, observer)`
  returning typed events with P1–P4 timestamps.
- `crates/renderer/src/shaders/skyglow.wgsl`,
  `crates/renderer/src/shaders/corona.wgsl` (new),
  `crates/renderer/src/stars.rs` (CPU cull).
- `crates/common/src/presets.rs`: `SolarEclipse`, `VenusTransit`,
  `LunarOccultationOfPlanet` presets. Existing `EclipseAid` stays lunar.
- CLI / viewer / web: HUD line showing active event + obscuration %;
  session JSON schema bump documented in `PROGRESS.md`.

**Tests / validation.** Pinned to entries logged in `VALIDATION.md`:
- Solar eclipses: 2009-07-22 (Tokara, total), 2012-05-21 (Tokyo,
  annular, ✅ pinned), 2024-04-08 (Mazatlán, total, ✅ pinned),
  2035-09-02 (Utsunomiya, predicted total) — Sun ↔ Moon separation
  within 1′ of NASA canon; P1–P4 within 30 s once `L-06` (DE440
  upgrade) lands. Current VSOP87 / ELP2000 stack pins detection,
  classification, and totality-duration plausibility; sub-30 s
  P1–P4 against TP-2006-214141 stays gated on DE440.
- Lunar eclipses: 2000-01-21, 2025-09-08 — umbral contact times within
  30 s.
- Lunar occultations: at least two IOTA-published star occultations
  within 5 s of predicted disappearance time.
- Transits: 2012-06-06 Venus transit, 2032-11-13 Mercury transit —
  ingress / egress times within 60 s.
- Visual: deterministic renders for partial / annular / total solar,
  total lunar, Venus-transit, and a star occultation added to the
  gallery, regenerated by `./scripts/generate-readme-images.sh`.
- Perf: `cargo bench` scene set must not regress beyond noise vs the
  pre-V-51 baseline; the analytic-mask path is the contract.

**Deliberate non-goal scope.** No prominence / chromosphere structure,
no polarized K-corona separation, no shadow-band atmospheric
scintillation, no global umbral-path map over Earth, no Jovian /
Saturnian moon eclipses (their parent moons are not rendered), no
asteroidal star occultations (asteroids not rendered). Documented in
`docs/standards-compliance.md` alongside the existing lunar-eclipse aid
caveat.

**Dependencies.** Tightens with `L-06` (DE440 / VSOP87 upgrade) for
sub-arcsecond contact timing; usable before that with the existing
~1″-class apparent positions. Subsumes the geometry half of `V-36`
(visual aid kept as-is for back-compat).

**Hosts wired.** CLI / viewer / web.

---

## Solar system depth

### `V-52` Planetary rings and moons — ⬜

**Item.** Today `V-35` renders Mercury–Neptune as bare disks. Three
visually unmistakable elements are missing: **Saturn's ring system**,
the **Galilean moons** (Io, Europa, Ganymede, Callisto), and **Titan**.
Without them, telescope-eyepiece scenes (`V-43`) look obviously wrong.

**Scientific basis.**
- Saturn ring geometry: inner / outer radii from Cassini fits (Porco et
  al. 2005); ring opening angle (B) from sub-Earth latitude of Saturn's
  pole; A / B / Cassini Division / C-ring brightness ratios from
  Dones et al. 1993.
- Galilean moons: VSOP-style Sampson 1921 / Lieske E5 (Lieske 1998)
  theory for Jovicentric positions, projected to topocentric apparent
  positions; magnitudes from JPL Horizons-style geometric formulae.
- Titan: TASS1.7 (Vienne & Duriez 1995) for Saturnicentric position.

**References.**
- Lieske, J. H. 1998, A&AS 129, 205 (E5 theory of Galilean moons).
- Vienne, A., Duriez, L. 1995, A&A 297, 588 (TASS1.7 Saturnian moons).
- Porco, C. C. et al. 2005, Science 307, 1226 (Cassini ring geometry).
- Dones, L. et al. 1993, Icarus 105, 184 (ring photometric profiles).

**Implementation scope.**
- `crates/astronomy/src/moons.rs` (new): `apparent_galilean_moons`,
  `apparent_titan` returning topocentric `(RA, Dec, magnitude,
  shadow_on_planet)`.
- `crates/astronomy/src/ephemeris.rs`: extend Saturn state with ring
  opening angle `B` and `B'` (illumination side).
- `crates/renderer`: Saturn-ring shader (oriented ellipse with A / B / C
  bands and Cassini gap, shadowed by the planet body); moon sprites as
  point lights with their own glare from the existing pipeline; shadow
  transits drawn via the analytic-mask path of `V-51b`.
- Reuse `V-51a/b` occultation primitives for mutual Galilean events
  (Io occulting Europa, shadow transits on Jupiter).

**Tests / validation.** Galilean configurations at known epochs within
5″ of JPL Horizons; Saturn ring opening at solstices / equinoxes within
0.1°; one deterministic eyepiece-sim render of Jupiter + 4 moons and
one of Saturn + rings added to the gallery.

**Deliberate non-goal scope.** No irregular moons of any planet, no
Neptunian / Uranian rings (faint, requires deep-field telescope sim),
no surface textures — the bodies stay as photometric point / disk
sources with magnitudes.

**Hosts wired.** CLI / viewer / web.

---

### `V-53` Resolved star clusters — ⬜

**Item.** Bright open clusters (Pleiades M45, Hyades, Praesepe M44,
Double Cluster, Beehive) currently appear as a single DSO label from
`V-42`. They should appear as a **resolved field of HYG / Hipparcos
stars** with the cluster label drawn over the field, because that is
what a naked-eye observer actually sees.

**Scientific basis.** Cluster membership lists from WEBDA / Cantat-Gaudin
2020 (Gaia DR2/DR3 membership catalog); the stars themselves are
already in HYG — only the membership tagging is new. No new photometry
model needed.

**References.**
- Cantat-Gaudin, T. et al. 2020, A&A 633, A99 (open-cluster membership
  from Gaia DR2).
- Mermilliod, J.-C., Paunzen, E. 2003 (WEBDA database).

**Implementation scope.**
- `crates/catalog/src/clusters.rs` (new): membership table keyed by HYG
  / Hip ID, joined into the loaded star slice.
- `crates/catalog/src/deepsky.rs`: when a DSO entry is tagged
  `resolve_as_member_field`, suppress the disk-only DSO sprite and
  rely on the star sprites for the visual; keep the label.
- `data/manifest.toml`: add cluster-membership snapshot with DOI.

**Tests / validation.** Pleiades render at 30′ FOV shows the 7 named
stars at the correct positions within 1′; one gallery image per
cluster.

**Deliberate non-goal scope.** No globular-cluster star-by-star
resolution (too dense, no per-member catalog at hobbyist scale — keep
as DSO disk). No cluster colour-magnitude diagrams.

**Hosts wired.** CLI / viewer / web.

---

### `V-54` Double / binary star resolution — ⬜

**Item.** Visual double / binary stars (Mizar / Alcor, Albireo,
Castor, ε Lyr "Double Double", Algieba, etc.) currently merge into
one HYG entry. At telescope-eyepiece zoom (`V-43`), close pairs must
split into two distinct sprites with correct separation, position
angle, and component magnitudes.

**Scientific basis.** Washington Double Star (WDS) catalog (Mason et al.
2001–) for separation `ρ`, position angle `θ`, and component
magnitudes per epoch. Apply per-component HYG colour from V-23.

**References.**
- Mason, B. D. et al. 2001, AJ 122, 3466 (WDS); USNO maintained
  updates.

**Implementation scope.**
- `crates/catalog/src/doubles.rs` (new): WDS-derived per-pair table
  keyed by HYG / Hip ID with `(ρ, θ, m1, m2)` at a documented epoch.
- `crates/catalog/src/catalog.rs`: when a HYG star matches a WDS
  primary, emit two sprites at `(ρ, θ)` from the primary; suppress
  the merged sprite.
- Acceptance threshold: split only when projected separation ≥ 1 px in
  the current FOV, otherwise fall back to the single merged sprite
  (no aliasing).

**Tests / validation.** Mizar / Alcor render at 1° FOV shows two
resolved sprites; Albireo eyepiece render shows the gold-blue colour
pair (V-23 photometry must agree); one gallery image.

**Deliberate non-goal scope.** No spectroscopic-binary modelling, no
orbital animation for short-period visual binaries (the catalog epoch
position is used as-is).

**Hosts wired.** CLI / viewer / web.

---

## Earth orbit

### `V-55` Artificial satellites (TLE / SGP4) — ⬜

**Item.** ISS, Starlink trains, the geostationary belt, and bright
LEO satellites are visible most clear nights and absent from the
renderer. Add a TLE-driven satellite layer propagated by SGP4, with
Earth-shadow visibility and apparent magnitude.

**Scientific basis.**
- Orbit propagation: SGP4 / SDP4 from Vallado et al. 2006 (the
  reference implementation that matches Space-Track distribution).
- Visibility: a satellite is naked-eye-visible when the observer is
  in night (Sun depression > civil) **and** the satellite is sunlit
  (not in Earth's umbra/penumbra). Reuse the umbra cone geometry
  already present for `V-36`.
- Magnitude: standard intrinsic-magnitude-at-1000-km approximation
  scaled by range and phase angle (McCants / Mike's Satellite Tracking
  convention) for amateur-grade values. Per-satellite intrinsic
  magnitudes from McCants' QuickSat MCNAMES file.

**References.**
- Vallado, D. A. et al. 2006, AIAA 2006-6753, *Revisiting Spacetrack
  Report #3* (SGP4 reference).
- Hoots, F. R., Roehrich, R. L. 1980, Spacetrack Report #3 (original
  SGP4 / SDP4).
- CelesTrak (celestrak.org) for current TLE feeds.
- McCants, M. (mmccants.org) for satellite intrinsic-magnitude file.

**Implementation scope.**
- `crates/astronomy/src/satellites.rs` (new): TLE parser + SGP4
  propagator (likely vendored from a reviewed crate such as `sgp4`),
  returning `(ECI position, ECI velocity)` at a given epoch.
- TLE to topocentric: rotate ECI to TEME → ECEF via existing
  GMST/nutation → alt-az via existing observer transform.
- Earth-shadow test: reuse umbra cone from `V-36`'s Earth-shadow
  geometry.
- `crates/renderer`: satellite sprites as moving point lights
  (single-frame and **streak** mode when frame integration is on); the
  streak length is `apparent_angular_velocity × exposure_seconds`,
  exposing exposure as a session field.
- `data/manifest.toml`: snapshot of a curated TLE set (ISS, a handful
  of bright Starlink + Iridium + geostationary representatives) with
  epoch and source URL, plus a regeneration command. Live TLE fetch
  is an opt-in CLI flag, not a default — deterministic renders rely
  on the snapshot.
- `crates/common/src/presets.rs`: `IssPass` preset (a known ISS pass
  for a fixed observer).

**Tests / validation.**
- SGP4: position at known TLE epochs matches the AIAA 2006-6753
  reference test vectors to documented sub-km accuracy.
- Visibility: ISS pass over Tokyo on a pinned date shows correct
  rise / culmination / shadow-entry times within 5 s of Heavens-Above.
- Visual: deterministic ISS-pass render and a Starlink-train render
  added to the gallery.

**Deliberate non-goal scope.** No collision-conjunction analysis, no
cross-section / BRDF physical magnitude model (intrinsic-magnitude
table is the agreed approximation), no debris cloud rendering, no
live network fetch in the default render path (manifest-pinned
snapshot only). Document live-fetch caveats in
`docs/standards-compliance.md`.

**Dependencies.** Independent of `V-51` but shares the Earth-shadow
utility with `V-36`.

**Hosts wired.** CLI / viewer / web.

---

## Interactive UX

### `V-56` Object search, GoTo, and info panel — ⬜

**Item.** Today the loaded catalog data (HYG star magnitudes, spectral
classes, distances; Messier / NGC / IC IDs; planet ephemerides) has no
UI surface beyond labels. Add three connected UX primitives:

1. **Search box** — free-text lookup by name / Bayer / Flamsteed /
   HD / HIP / HR / Messier / NGC / IC / planet / Moon / Sun, with
   prefix + fuzzy match and a ranked dropdown.
2. **GoTo** — selecting a result slews the camera to centre the
   target at the current FOV (smooth interpolation in alt-az for
   Earth observers; great-circle in the projection's native frame
   for all-sky and external viewpoints).
3. **Info panel** — click / tap / hover on any rendered body opens a
   panel showing apparent magnitude, RA/Dec (J2000 + apparent),
   Alt/Az, distance, spectral / colour class, rise / transit / set
   (reusing `L-07`), and the deep-link block from `L-19` when wired.

**Scientific basis.** No new physics — every datum already exists in
`crates/catalog` and `crates/astronomy`. The contribution is an index
layer + a picking buffer.

**Implementation scope.**
- `crates/catalog/src/search.rs` (new): in-memory inverted index over
  catalog identifiers + common names, exposing
  `search(query, limit) -> [Match { id, kind, score, display }]`.
  Built once at catalog load, cost bounded by `L-16` backend.
- `crates/renderer/src/picking.rs` (new): cheap ID picking via an
  off-screen R32Uint attachment populated only when the host requests
  a pick; falls back to nearest-sprite CPU search when picking buffer
  is disabled (web-low-power path).
- `crates/common`: `SelectedTarget` session field so a session JSON
  records the currently-focused object.
- `apps/web`, `apps/viewer`: search box widget + info panel,
  bilingual (en/ja) consistent with the rest of the web UI.
- `apps/cli`: `--goto <id>` flag that centres the camera on a named
  target before rendering.

**Tests / validation.**
- Unit: search ranks `"vega"`, `"α Lyr"`, `"HIP 91262"`, `"M31"`,
  `"NGC 224"`, `"saturn"`, `"土星"` to the expected catalog row.
- Unit: GoTo on Vega from a fixed observer + epoch lands the centre
  ray within 0.5′ of the apparent position.
- Visual: one deterministic CLI render with `--goto m31` from Tokyo
  added to the gallery.

**Deliberate non-goal scope.** No natural-language query ("show me
bright red giants near Orion") — keyword + identifier match only.
No server-side search index — must work entirely in-process for the
WASM build.

**Dependencies.** Tightens with `L-17` / `L-18` (richer identifier
preservation) and `L-19` (SIMBAD / VizieR deep links) but ships
standalone against current HYG + DSO + planet labels.

**Hosts wired.** CLI / viewer / web.

---

# Library track

## Time and positional precision

### `L-01` Time systems — ✅ done

**Item.** Separate UTC / UT1 / TAI / TT and an approximate TDB exposed to
ephemerides. Hosts pick the right scale per call: UT1 for sidereal time,
TT for ephemerides, UTC for I/O.

**Reference.** USNO / IERS leap-second table; SOFA `iauUtctai` / `iauUt1tt`
families.

**Implementation.** Built-in leap-second table, explicit DUT1, TT, and
approximate TDB exposed through `astronomy::TimeScales` and wired into
every host.

**Tests / validation.** Pinned values at known epochs (J2000.0, 2010-01-01
UTC, leap-second insertion boundary).

---

### `L-02` IAU 2006 precession — ✅ done

**Item.** Star positions of date instead of J2000, so labels don't drift
50″/year.

**Reference.** IAU 2006 (P03) Fukushima-Williams precession matrix.

**Implementation.** `astronomy::corrections`, applied in the renderer
camera uniform `j2000_to_date`, consumed in `shaders/star.wgsl`.

**Tests / validation.** Vector-length invariant; pinned matrix values at
J2050, J2100; comparison against SOFA `iauPmat06`.

---

### `L-03` Compact nutation — ✅ done

**Item.** Compact IAU-2000-style dominant luni-solar nutation terms,
targeting the renderer-scale ~9″ budget.

**Reference.** IAU 2000A reduced; compact term selection following Meeus
1998, *Astronomical Algorithms*, Ch. 22.

**Implementation.** `astronomy::corrections` with equation-of-equinoxes
sidereal-time wiring.

**Tests / validation.** Δψ / Δε values at known epochs within the
documented budget of full IAU 2000A.

---

### `L-04` Annual aberration — ✅ done

**Item.** First-order annual aberration from Earth's orbital velocity,
uploaded per frame and applied in the star shader.

**Reference.** Meeus 1998, *Astronomical Algorithms*, Ch. 23.

**Implementation.** `astronomy::corrections`,
`crates/renderer/src/shaders/star.wgsl` via `aberration_pm` uniform.

**Tests / validation.** Aberration magnitude ≈ 20.5″ at quadrature;
direction matches Earth's instantaneous velocity vector.

---

### `L-05` Stellar proper motion — ✅ done

**Item.** Apply HYG's `pmra` / `pmdec` per frame so positions drift away
from the J2000 catalog epoch on multi-decade renders.

**Implementation.** `catalog::coords`, `catalog::catalog`,
`renderer::vertex`, `shaders/star.wgsl`. HYG `pmrarad` / `pmdecrad`
converted to Cartesian tangent vectors in both CSV and embedded paths.

**Tests / validation.** Pinned position of Barnard's Star at J2000 + 30 yr
vs. SIMBAD.

---

### `L-06` DE440 / VSOP87 ephemeris upgrade — ⬜

**Item.** Move Sun / Moon / planet states from the current VSOP87 /
ELP2000 visual-quality models to publication-quality JPL DE440 Chebyshev
kernels. Preserve a documented fallback to VSOP87 / ELP2000 for offline /
lightweight builds (WASM in particular).

**Scientific basis.** DE440 (Park et al. 2021, AJ 161, 105) is JPL's
current planetary + lunar ephemeris, accurate to ~0.1 mas for the inner
planets over the renderer's epoch range. JPL distributes binary kernels;
the parser pattern is well-established (Acton 1996 SPICE; Spiceypy /
jplephem reference implementations).

**References.**
- Park, R. S. et al. 2021, AJ 161, 105 (DE440 / DE441).
- Acton, C. H. 1996, Planet. Space Sci. 44, 65 (SPICE toolkit).

**Implementation scope.**
- `crates/astronomy/src/ephemeris.rs`: feature `de440` builds a Chebyshev
  kernel reader; default to a slim DE440 subset (Sun, Moon,
  Mercury–Neptune; modern epoch) committed to the data manifest, with
  an optional full-kernel path.
- WASM build keeps VSOP87 / ELP2000 because DE440 kernels are large.
- Light-time iteration + planetary-aberration loop reused from the
  current VSOP87D path.

**Tests / validation.**
- Cross-check Sun, Moon, Mars against JPL Horizons at fixed epochs;
  target < 1″ topocentric for naked-eye epochs.
- Document the DE440 commit subrange and the fallback path's stated
  precision tier in `docs/standards-compliance.md`.

**Hosts wired.** CLI / viewer (full); web (VSOP87 / ELP2000 fallback).

**Visual side-effect.** Sun / Moon / planet apparent positions used by
`V-30`, `V-35`, `V-36`, `V-49` get a precision bump; user-visible only at
sub-arcminute scales.

---

## Observation planning helpers

### `L-07` Rise / transit / set tables — ✅ done

**Item.** Per object, per evening, in the planning UI.

**Implementation.** `astronomy::planning`, `apps/web/frontend`.

**Tests / validation.** Cross-check against USNO rise / transit / set
tables for representative cities and dates.

---

### `L-08` Twilight indicators — ✅ done

**Item.** Civil / nautical / astronomical twilight bands shown in the
planning panel and as solar-depression labels.

**Implementation.** `astronomy::planning`, `apps/web/frontend`.

**Tests / validation.** Cross-check against NOAA solar-calculator
twilight times.

---

### `L-09` Observation-planning polish — ⬜

**Item.** Favourites, "tonight's recommended objects", Moon-impact score,
visibility score, and optional calendar export, all derived from
documented planning helpers.

**Scientific basis.** Moon-impact score follows Krisciunas-Schaefer 1991
moonlight sky-brightness contribution at the target's altitude. Visibility
score combines altitude-vs-time, twilight bands, and (when present from
`V-39`) site sky brightness.

**Implementation scope.**
- `crates/astronomy/src/planning.rs`: scoring helpers with pinned
  reference values.
- `apps/web/frontend`: planning panel UX (favourites, recommended-list,
  scores).
- iCalendar export of the chosen targets and the local visibility
  windows.

**Tests / validation.**
- Unit: Moon-impact for a target at 20° altitude with the Moon at 60°
  altitude and 90% illumination matches the K-S-derived ΔV.
- Visual: planning panel screenshots for two reference dates.

**Hosts wired.** Web (primary UX); CLI exports JSON of the same
calculations.

---

## Sessions and reproducibility

### `L-10` Session URL — ✅ done

**Item.** Encode `(lat, lng, jd, az, alt, fov, overlays, planets,
projection, atmosphere preset)` in a shareable URL.

**Implementation.** `apps/web/frontend`. URL load / copy path using plain
query parameters; no version gate.

**Tests / validation.** Roundtrip tests; documented in
`docs/scene-presets.md`.

---

### `L-11` Sharable JSON sessions — ✅ done

**Item.** Schema-versioned JSON sessions covering observer, time scales,
view, overlays, projection / viewpoint, active corrections, atmosphere,
catalog snapshot, eyepiece, and app version, across CLI, desktop, and web
hosts.

**Implementation.** `stars_host_common::session`, `apps/{cli,viewer,web}`.

**Tests / validation.** Roundtrip tests across hosts; schema-version
bumps documented in the session module.

---

### `L-12` Scene presets — ✅ done

**Item.** Deterministic named scenes for Tokyo tonight, dark sky, noon,
sunset, civil / nautical / astronomical twilight, moonlit night, eclipse
aid, all-sky maps, and external galactic viewpoints.

**Implementation.** `stars_host_common::presets`, `apps/{cli,viewer}`.

**Tests / validation.** Each preset has a pinned PNG in the gallery and a
session roundtrip test.

---

### `L-13` Notebook examples — ✅ done

**Item.** Reproducible examples that load JSON sessions, compare tabular
astronomy outputs, and render the same scene as web / CLI.

**Implementation.** `examples/notebooks`,
`apps/cli/examples/session_table.rs`.

**Tests / validation.** Notebook smoke test wired into `make ci`.

---

### `L-14` Public demo gallery — ⬜

**Item.** Curated, shareable scenes such as Tokyo tonight, summer Milky
Way, lunar eclipse aid, and galactic-north view, backed by stable session
files.

**Scientific basis.** Demo discipline: each scene is reproducible and
cites the chosen catalog, atmosphere, and ephemeris versions through the
session schema.

**Implementation scope.**
- `docs/public-gallery.md` index; each entry pinned to a committed
  session JSON.
- A small static site under `apps/web/frontend/gallery/` that links to
  the live render with the session preloaded.
- Curation policy: every gallery entry has a citation block.

**Tests / validation.**
- `make ci` re-renders the gallery scenes and diffs against the
  committed PNGs.

**Hosts wired.** Web (gallery page); CLI (re-render); viewer (load via
session).

---

### `L-15` Data provenance manifest — ✅ done

**Item.** `data/manifest.toml` records every committed data artifact,
generated artifact, and runtime web service with SHA-256, source,
licence, version, preprocessing command, and fields used.
`stars-manifest` parses and verifies the schema, and
`make manifest-check` (wired into `make ci`) re-hashes every artifact so
unrecorded drift fails CI.

**Implementation.** `crates/manifest`, `data/manifest.toml`.

**Tests / validation.** `make manifest-check` is the test.

---

## Catalog backend and identifiers

### `L-16` Catalog backend scaling design — ✅ done

**Item.** Document the backend trait, identifier mapping, LOD / spatial
index strategy, streaming / paging, and the small embedded WASM subset
before the large-catalog ingest in `L-17`.

**Implementation.** `docs/catalog-backend-design.md`, plus the scaffold
`catalog::CatalogBackend` + `catalog::HygCsvBackend` in `crates/catalog`.

**Tests / validation.** Trait-level tests cover the HYG CSV and embedded
backends.

---

### `L-17` Hipparcos / Tycho-2 / Gaia DR3 ingest — ⬜

**Item.** Pluggable catalog backend that supports Hipparcos (118 k stars),
Tycho-2 (2.5 M), and Gaia DR3 (1.8 B) at increasing precision tiers while
keeping HYG as the embedded WASM build's default.

**Scientific basis.** Each catalog has documented precision and
completeness: Hipparcos (Perryman 1997, A&A 323, L49) ≈ 1 mas; Tycho-2
(Høg 2000, A&A 355, L27) ≈ 60 mas; Gaia DR3 (Gaia Collaboration 2022,
A&A 674, A1) ≈ 20 μas for bright stars. Switching catalogs at the backend
level (rather than concatenating) is required to preserve their
documented zero-points.

**Implementation scope.**
- `crates/catalog`: backend implementations behind the trait from
  `L-16`. LOD streaming for Gaia (the full Gaia DR3 source is too large
  to embed; stream from a content-addressable store).
- `data/manifest.toml`: each catalog appears as a row with SHA-256 of
  the canonical archive, source DOI, licence, version, and preprocessing
  command.
- Bench coverage so the LOD / cull path is shown not to blow up frame
  time on Tycho-2.

**Tests / validation.**
- Unit: identifier round-trips (HIP → Tycho → Gaia source_id) where
  cross-IDs exist.
- Cross-catalog comparison: Sirius J2000 position from Hipparcos vs.
  HYG vs. Gaia DR3 within their stated tolerances.

**Hosts wired.** CLI / viewer / web (web defaults to embedded HYG; LOD
streaming optional behind a setting).

---

### `L-18` Identifier preservation — ⬜

**Item.** Pass Hipparcos / HD / TYC / Gaia source_id through the renderer
so hover, click-to-copy, session reproducibility, SIMBAD / VizieR deep
links, and catalog snapshots all reference the same star by its canonical
ID.

**Scientific basis.** Astronomical reproducibility depends on stable
identifiers; the catalogs above publish them under documented conventions
(Hipparcos: HIP nnnnn; HD: HD nnnnnn; Tycho: TYC nnnn-nnnnn-n; Gaia:
source_id u64).

**Implementation scope.**
- `crates/catalog`: per-star optional ID record (`StarIdentifiers`)
  flowing through `build_star_instance` into `renderer::vertex`.
- `crates/common`: JSON session encodes the chosen primary ID family.
- `apps/*`: hover tooltip + click-to-copy use the primary ID family.

**Tests / validation.**
- Unit: Sirius identifiers round-trip end-to-end across CSV / embedded /
  WASM paths.
- Cross-check: hover ID matches `L-17` cross-ID test outputs.

**Hosts wired.** CLI metadata JSON / viewer hover / web hover.

---

### `L-19` SIMBAD / VizieR deep links — ⬜

**Item.** Hover a star → external link with the right SIMBAD / VizieR
query. Keep external services optional and out of deterministic renders
(no network call from the rendering pipeline).

**Scientific basis.** SIMBAD / VizieR are the canonical CDS lookup
services for stellar metadata; their query URL formats are documented
(Wenger 2000, A&AS 143, 9; Ochsenbein 2000, A&AS 143, 23).

**Implementation scope.**
- `crates/common`: helper `simbad_query_url(ids: &StarIdentifiers) ->
  String`.
- `apps/web/frontend`: hover panel renders the link; opt-in toggle.
- No network calls from `crates/renderer`; the link is just a URL.

**Tests / validation.**
- Unit: URL format matches CDS specification for a representative ID set.
- Browser test on the hover panel (mocked navigation).

**Hosts wired.** Web (CLI / viewer expose the URL in metadata JSON).

---

### `L-20` Variable star light curves — ⬜

**Item.** Side-panel light curve for a hovered variable star, with
source, epoch, and uncertainty caveats. Renderer brightness optionally
reflects the variable's current state at the session time (Mira / Algol
visibly change between epochs).

**Scientific basis.** AAVSO VSX is the canonical variable catalogue
(Watson 2006, SASS 25, 47). Period / epoch / amplitude / type fields are
standardised; the predicted current magnitude at session time uses
elements (T₀, P, light-curve shape) per AAVSO conventions.

**References.**
- Watson, C. L. 2006, SASS 25, 47 ("The International Variable Star
  Index (VSX)").
- AAVSO Variable Star Index (live; pinned snapshot per manifest).

**Implementation scope.**
- `crates/catalog/src/variables.rs`: VSX snapshot + element evaluation.
- `apps/web/frontend`: side panel renders a small canvas light curve and
  the literature reference.
- `crates/renderer/src/vertex.rs`: optional `current_magnitude_override`
  per variable star, default off to preserve catalogue purity.

**Tests / validation.**
- Unit: Algol primary minimum at a documented epoch returns expected
  Δm.
- Visual: side panel shows correct curve and reference text for Mira at
  a fixed session time.

**Hosts wired.** Web first (visualisation surface); CLI exports the
predicted Δm in metadata JSON; viewer follows.

---

## Bindings and hosts

### `L-21` Python bindings (PyO3) — ⬜

**Item.** `astronomy` + `catalog` callable from Jupyter via PyO3 / maturin,
so a notebook can reproduce the same numbers the rendering pipeline
consumes. Early notebook examples in `examples/notebooks` use CLI renders
+ JSON sessions until full bindings land.

**Scientific basis.** Reproducibility-by-binding: a teacher / reviewer
should be able to call the exact functions the renderer calls.

**Implementation scope.**
- New crate `crates/pyastronomy` exposing the `astronomy` public API
  through PyO3.
- `crates/pycatalog` similarly for catalog access.
- `pip install stars-astronomy` wheel built via maturin in CI for
  Linux / macOS / Windows.
- Notebook examples in `examples/notebooks` consume the bindings and
  match the corresponding CLI session render.

**Tests / validation.**
- Bind-time tests on representative functions
  (`magnitude_to_illuminance_lux`, `airmass_kasten_young`,
  `precession_iau2006`).
- Notebook smoke test as part of `make ci` (or a documented
  python-extras gate).

**Hosts wired.** Bindings live alongside the existing hosts; not a host
itself.

---

### `L-22` Headless server mode — ⬜

**Item.** HTTP service that returns PNGs and metadata JSON from a supplied
scene / session. Already ~90% there in `apps/cli`; the missing piece is
the HTTP envelope + content negotiation.

**Implementation scope.**
- New crate / binary `apps/server` reusing `apps/cli`'s render core.
- Endpoints: `POST /render` (JSON session in, PNG out, optional metadata
  JSON), `GET /healthz`.
- No external network calls from the render path; observer geocoding
  stays opt-in client-side.

**Tests / validation.**
- Integration test: POST a known session, assert PNG SHA-256.
- Stress test with concurrent requests (light load; the goal is
  reproducibility, not high throughput).

**Hosts wired.** Server is its own host; CLI / viewer / web unchanged.

---

## Education and accessibility

### `L-23` Guided education mode — ⬜

**Item.** Cross-host tour content explaining horizon, equator, ecliptic,
galactic plane, time motion, twilight, and projection choices. Drives the
renderer through documented sequences rather than ad-hoc panning.

**Scientific basis.** Pedagogical sequencing; tour steps reference the
literature that motivates each overlay (e.g., the equation of time, the
analemma, the obliquity of the ecliptic).

**Implementation scope.**
- `crates/common`: `Tour { steps: Vec<TourStep> }` schema; each
  `TourStep` has session-delta + caption + optional reference URL.
- Hosts render the caption in their native UI; the renderer itself
  doesn't know about tours.
- A built-in "first night" tour ships in `apps/{cli,viewer,web}`.

**Tests / validation.**
- Roundtrip a sample tour through JSON.
- Each step must reduce to a deterministic render (no time-of-walltime
  dependence).

**Hosts wired.** CLI / viewer / web.

---

### `L-24` Accessibility pass — ⬜

**Item.** ARIA labels on every web control, keyboard navigation,
high-contrast / colour-vision-safe modes, screen-reader summaries, and
optional Az / Alt audio cues.

**Scientific basis.** WCAG 2.2 AA. Colour-blind safe palettes following
Wong 2011, Nature Methods 8, 441.

**References.**
- W3C WCAG 2.2 (current).
- Wong, B. 2011, Nature Methods 8, 441 ("Points of view: Color
  blindness").

**Implementation scope.**
- `apps/web/frontend`: ARIA on every gear / menu / slider; full keyboard
  flow.
- Renderer: high-contrast / CVD-safe overlay palette selectable via
  `OverlayConfig`.
- Optional audio: Web Audio API tones for Az / Alt indicator (off by
  default).

**Tests / validation.**
- Lighthouse / axe-core scores ≥ documented threshold.
- Manual screen-reader pass on the gear menu and tour mode.

**Hosts wired.** Web first; CLI / viewer follow with platform-native
accessibility hooks.

---

## Citation and standards traceability

### `L-25` `CITATION.cff` + Zenodo DOI — ✅ done

**Item.** `CITATION.cff`, `.zenodo.json`, `docs/citation.md` define
preferred citation text, release-DOI workflow, and data / source caveats.

**Implementation.** Repo root + `docs/`.

**Tests / validation.** Manual review on release; Zenodo webhook covered
in repo settings.

---

### `L-26` Standards-compliance document — ✅ done

**Item.** `docs/standards-compliance.md` lists implemented IAU / SOFA-
aligned constants and routines, intentional approximations, and
deliberate non-goals.

**Implementation.** `docs/standards-compliance.md`. Every new model row
in this roadmap updates that document in the same PR.

**Tests / validation.** Review discipline enforces sync.

---

### `L-27` Validation / demo gallery — ✅ done

**Item.** Render preset PNGs with fixed inputs; publish a human gallery
and run perceptual or tolerance-based screenshot comparisons where CI
can do so reliably.

**Implementation.** `docs/validation-gallery.md`,
`scripts/render-validation-gallery.sh`.

**Tests / validation.** Gallery regenerator script is opt-in; perceptual
tolerance documented in the validation gallery.
