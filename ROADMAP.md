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
   (Bortle / SQM / Falchi atlas). ✅ (`V-39` Bortle + SQM core plus the
   `V-39-Atlas` Falchi 2016 World Atlas loader, sampling zenith brightness by
   observer lat/lng).

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
remaining items are realism polish (`V-24` scintillation, `V-25`–`V-28`
have all shipped), site-specific brightness (Bortle / SQM core shipped
via `V-39`, including the `V-39-Atlas` Falchi 2016 World Atlas loader),
niche visual
features (`V-45` telescope-side optical artifacts, `V-46` galactic structural
model, and `V-50` output colour management have all shipped), and rare
phenomena (`V-47` meteor showers has shipped; `V-48`–`V-49` remain).

**High priority next:** the visual-richness gaps `V-51`–`V-56` (eclipses /
occultations, planetary rings and moons, resolved star clusters, double
stars, artificial satellites, and object search / GoTo / info panel) —
these surface existing engine capability that the current UI hides. The
unified eclipse / occultation pass `V-51` has now shipped end-to-end:
common occultation primitives (`V-51a`), the general N≤16 occluder
uniform array (`V-51b`), the solar-eclipse renderer path (`V-51c`),
lunar occultation of stars and planets (`V-51d`), Mercury / Venus
transits of the Sun (`V-51e`), and mutual planetary occultation
(`V-51f`). `V-52` has now shipped end-to-end: the Saturn ring system
(`V-52a`), the Galilean moons (`V-52b`) upgraded to the full Lainey
2006 L1.2 series (`V-52b-E5`), Titan (`V-52c`) upgraded to the full
Vienne & Duriez 1995 TASS1.7 series (`V-52c-TASS17`), and Galilean
shadow transits + moon-behind-Jupiter culling (`V-52d`). Object search /
GoTo / info panel (`V-56`) is now wired across all three hosts: the web
search box / dropdown / click-pick info panel, a CLI `--goto <name>`
flag, and an interactive `/`-triggered viewer search prompt with a
title-bar info panel — so `V-56` has shipped. `V-54` (double / binary
star resolution) has now shipped: a WDS-derived bootstrap table resolves the
visual doubles HYG merges into one row (Algieba and the ε Lyrae "Double
Double") into per-component sprites at catalog-load time, so all three hosts
get the split for free. `V-55` (artificial satellites, TLE / SGP4) has also
shipped end-to-end: an `astronomy` SGP4 propagator + topocentric /
Earth-shadow / magnitude pipeline, a renderer satellite point/streak layer, a
manifest-pinned curated TLE snapshot, an `iss-pass` preset, and CLI / viewer /
web host controls. With both shipped, every `V-51`–`V-56` item is now done.

The Library track is at "amateur-grade is shipped" — the remaining items are
DE440-class ephemerides (`L-06`), large catalog ingest (`L-17`), bindings and
headless server (`L-21`, `L-22`), variable-star library (`L-20`), and
education / accessibility (`L-23`, `L-24`). Observation-planning polish
(`L-09`) has now shipped: Moon-impact and visibility scoring, recommended-
object ranking, favourites, and iCalendar export.

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
| `V-25` | **Differential atmospheric dispersion** | ✅ |
| `V-26` | Lunar earthshine | ✅ |
| `V-27` | **Belt of Venus + Earth-shadow band** | ✅ |
| `V-28` | **Spectral airglow decomposition** | ✅ |
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
| `V-39` | Light pollution / Bortle + SQM + Falchi 2016 World Atlas loader | ✅ |
| `V-40` | Full-sky projections (Mollweide / Aitoff / Hammer) | ✅ |
| `V-41` | Out-of-Earth galactic-north viewpoint | ✅ |
| `V-42` | Deep-sky overlay (Messier + bright NGC / IC subset) | ✅ |
| `V-43` | Telescope eyepiece simulation | ✅ |
| `V-44` | Custom external viewpoint origin | ✅ |
| `V-45` | **Telescope-side optical artifacts** | ✅ |
| `V-46` | **Galactic structural model for external viewpoints** | ✅ |
| `V-47` | **Meteor shower display** | ✅ |
| `V-48` | **Aurora display** | ⬜ |
| `V-49` | **Comet rendering** | ⬜ |
| `V-50` | **Output colour management (sRGB / P3 / Rec.2020)** | ✅ |
| `V-51` | **Unified eclipse / occultation pass** (`a` + `b` + `c` + `d` + `e` + `f` done) | ✅ |
| `V-52` | **Planetary rings and moons (Saturn / Galilean / Titan)** (all rungs `a`–`d` done; Galilean at Lainey 2006 L1.2, Titan at TASS1.7) | ✅ |
| `V-53` | **Resolved star clusters (Pleiades, Hyades, …)** (showpiece bootstrap done) | ✅ |
| `V-54` | **Double / binary star resolution** | ✅ |
| `V-55` | **Artificial satellites (TLE / SGP4)** | ✅ |
| `V-56` | **Object search, GoTo, and info panel** | ✅ |

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
| `L-09` | Observation-planning polish | ✅ |
| `L-10` | Session URL encoding | ✅ |
| `L-11` | Sharable JSON sessions | ✅ |
| `L-12` | Scene presets | ✅ |
| `L-13` | Notebook examples | ✅ |
| `L-14` | Public demo gallery | ✅ |
| `L-15` | Data provenance manifest | ✅ |
| `L-16` | Catalog backend scaling design | ✅ |
| `L-17` | Hipparcos / Tycho-2 / Gaia DR3 ingest | ⬜ |
| `L-18` | Identifier preservation through the renderer | ⬜ |
| `L-19` | SIMBAD / VizieR deep links | ✅ |
| `L-20` | Variable star light curves | ⬜ |
| `L-21` | Python bindings (PyO3) | ⏳ astronomy queries + session round-trip shipped |
| `L-22` | Headless server mode | ✅ |
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

### `V-25` Differential atmospheric dispersion — ✅

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

### `V-26` Lunar earthshine — ✅ done

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

### `V-27` Belt of Venus and Earth-shadow band — ✅

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

### `V-28` Spectral airglow decomposition — ✅

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

### `V-39` Light pollution / Bortle map — ✅ done (Bortle / SQM core + Falchi 2016 atlas loader)

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

**Hosts wired.** CLI / viewer / web (web WASM setter `set_light_pollution`
is in place; the React settings card is deferred to a follow-up PR).

**Shipped (core slice).**
- `astronomy::skyglow::LightPollution { Bortle(u8), Sqm(f32), Atlas2016 {
  latitude_deg, longitude_deg } }`. Bortle 1..=9 → SQM lookup pinned to the
  Bortle 2001 / Cinzano-Falchi-Elvidge 2001 typical zenith table (Class 5
  anchored at V = 20.0 to satisfy the V-39 calibration test).
- `astronomy::skyglow::garstang_zenith_distance_kernel` (single-scattering
  zenith-distance kernel, clamped at 85° so horizon pixels do not blow up);
  `artificial_skyglow_s10` returns the S10 excess per pixel.
- Renderer: `Camera::light_pollution`, `CameraUniform::light_pollution_state`,
  `CameraUniform::light_pollution_tint`; the WGSL skyglow pass adds the
  artificial term *before* extinction with the sodium / LED warm-orange tint.
- Session schema bumped to **v5** (`SessionLightPollution { kind, bortle?,
  sqmMagPerArcsec2?, atlasLatitudeDeg?, atlasLongitudeDeg? }`).
- CLI / viewer flags: `--bortle`, `--sqm`, `--light-pollution-atlas LAT LNG`,
  `--no-light-pollution`. WASM `set_light_pollution(enabled, kind, ...)`.
- Gallery presets: `tokyo-bortle-8` (Bortle 8 + hazy-urban atmosphere) and
  `dark-sky-bortle-1` (byte-identical to `dark-sky` by construction).

**Shipped (`V-39-Atlas` follow-up).**
- `astronomy::FalchiAtlas` (`light_pollution_atlas.rs`): an IO-free parser +
  bilinear sampler for a compact `FALATL01` lat/lng grid of total zenith V
  mag/arcsec², resampled from the Falchi 2016 World Atlas. NaN no-data cells
  (ocean / out-of-swath) are skipped in the blend.
- `scripts/fetch-falchi-atlas.sh` downloads the ~1 GB upstream GeoTIFF (DOI
  10.5880/GFZ.1.4.2016.001, licence accepted at the landing page) and
  `scripts/build-falchi-atlas.py` resamples it to the `FALATL01` grid using the
  flux-additive model `μ = 21.6 − 2.5·log10(1 + ratio)` (the renderer's natural
  floor). The GeoTIFF is recorded in `data/manifest.toml` as a non-committed
  external dataset.
- Native hosts resolve `LightPollution::Atlas2016 { lat, lng }` at render time
  via `stars_host_common::resolve_light_pollution`, loading the grid named by
  `STARS_FALCHI_ATLAS` and sampling it into an equivalent
  `LightPollution::Sqm`. The session keeps the `Atlas2016` (lat, lng) for
  reproducibility, and the variant falls back to the Bortle-1 floor when no
  grid is configured, so the default render path stays deterministic and the
  renderer / shader / schema are unchanged.

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

### `V-45` Telescope-side optical artifacts — ✅ done

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

**Implementation.**
- `crates/renderer/src/camera.rs` (the eyepiece model lives here, not a
  separate `eyepiece.rs`): `OpticalDesign { Refractor { achromat: bool,
  focal_ratio }, Newtonian { spider_vanes: u8 }, SchmidtCassegrain {
  obstruction_pct } }` with `central_obstruction_ratio`, `spider_vanes`,
  `achromat_focal_ratio`, and kebab parse/emit helpers. `EyepieceSimulation`
  gains an `optical_design` and an `ota_rotation_deg` (appended at the end of
  the struct), plus `airy_radius_rad(λ) = 1.22 λ/D` and a
  `chromatic_fraction()` Conrady-style secondary-spectrum scale. Two
  `instrument_optics*` rows are appended at the end of `CameraUniform` and
  populated only when the eyepiece is active in a perspective Earth view
  (`airy_radius_px`, obstruction ratio, vane count, spike angle, enabled,
  chromatic fraction, vignette strength).
- `crates/renderer/src/shaders/star.wgsl`: the bright-star PSF composites
  the Spencer eye PSF with an instrument PSF — an obstructed-aperture Airy
  pattern (`2J1(x)/x` via an Abramowitz & Stegun J1 approximation, annular
  for the central obstruction), spider diffraction spikes (one bidirectional
  ray per vane, so even vane counts give `n` arms and odd counts `2n`),
  per-channel chromatic ring shift for achromats, and an exit-pupil-relative
  cos⁴ vignette. Spikes rotate with `ota_rotation_deg`. Outside eyepiece
  mode the branch is skipped and the PSF is bit-identical to the
  naked-eye pipeline. `skyglow.wgsl` is untouched.
- Hosts: CLI `--telescope-design` / `--spider-vanes` / `--ota-rotation-deg`;
  viewer `O` (cycle design), `[` / `]` (roll OTA) keybinds plus the same
  flags; web settings-panel design dropdown / vane / rotation controls,
  the `set_telescope_optics` WASM binding, URL params, and localStorage.

**Tests / validation.**
- Unit: Airy radius at D=200 mm, λ=550 nm is 0.69″ within 1%
  (`airy_radius_matches_born_and_wolf`); obstruction/vane mapping per
  design; achromat chromatic fraction is zero for apochromats and larger
  for faster achromats; the instrument uniform is disabled outside eyepiece
  mode and the Airy pixel radius grows with magnification.
- A naga WGSL parse + validate test (`shaders_parse_and_validate`) runs in
  CPU-only CI, guarding the shader and the Rust↔WGSL `CameraUniform`
  layout (CI has no GPU to validate at pipeline creation).

**Hosts wired.** CLI / viewer / web (when eyepiece mode is active). The
optical design is a live render control this cycle; persisting it in the
shared JSON session schema is a documented follow-up (it can append
optional fields without a schema bump).

---

### `V-46` Galactic structural model for external viewpoints — ✅ done

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

**Implementation.**
- `crates/astronomy/src/galaxy.rs`: closed-form
  `milky_way_luminosity_density(x_pc, y_pc, z_pc)` (thin disk with a
  sech² vertical profile + Reid 2019 four-arm log-spiral enhancement,
  exponential thick disk, and a rotated triaxial boxy bar) plus
  `dust_extinction_az(distance_pc, l_rad, b_rad)` (double-exponential
  dust disk integrated along the line of sight). Galactocentric IAU
  parsecs, Sun on +x at `R_SUN_PC = 8122`, `Z_SUN_PC = 20.8`. Arm ridge
  anchors (`SPIRAL_ARMS`) carry the Reid 2019 pitch angles.
- `crates/renderer/src/shaders/skyglow.wgsl`: the external-viewpoint
  branch (`external_galaxy_disc_radiance`) now emission-absorption
  ray-marches `gal_density` attenuated by the dust disk
  (`gal_dust_density`) instead of intersecting a single analytic plane,
  so the bar, arms, and dark dust lanes resolve. Constants mirror
  `astronomy::galaxy` value-for-value (kept in lock-step like the ISL
  model); the pinned Rust tests guard the shared model.
- No new host control, session field, or `CameraUniform` slot: the
  existing external viewpoint toggle (`V-41` / `V-44`) drives the upgrade
  across CLI / viewer / web.
- The model is closed-form (no external data artifact), so no
  `DATA_SOURCES.md` / manifest row is required.

**Tests / validation.**
- Unit (pinned): solar position (8.122 kpc, 0, 0.0208 kpc) returns local
  stellar density ≈ 0.1 M_sun pc⁻³; density falls with `|z|` and with `R`;
  the bar dominates the centre by ≫ 50×; an on-arm point exceeds the
  inter-arm density; dust extinction is zero at zero distance, monotone
  in distance, plane-concentrated, and ≈ 1 mag/kpc locally.
- Visual: external view from above the Sun shows the four arms and the
  central bar in the correct azimuths with dark dust lanes.

**Hosts wired.** CLI / viewer / web (via the existing external viewpoint).

---

## Rare and transient phenomena

### `V-47` Meteor shower display — ✅

**Item.** Stochastic meteor rendering, anchored to the IMO Working List
of Visual Meteor Showers (radiant α / δ, peak date, ZHR, population index
r, atmospheric velocity v_∞). On-screen meteors appear from the radiant
at the configured rate, with deterministic seeding so the same JSON
session reproduces the same meteor stream.

**Scientific basis.** Koschack & Rendtel 1990 give the visual ZHR
reduction `ZHR = n · F · r^(6.5 − lm) / sin(h_R)`. Inverting it for the
*observed* rate a real sky produces gives `n = ZHR · sin(h_R) ·
r^(lm − 6.5) / F`, which is what the renderer samples — note `lm < 6.5`
correctly *lowers* the rate below ZHR (a darker-than-standard sky shows
fewer meteors), so the worked Perseid example below lands at ≈ 58 m/h,
not the optimistic ≈ 100 m/h of the original sketch. Date-dependent ZHR
follows the Jenniskens 1994 solar-longitude activity profile
`ZHR(λ) = ZHR_max · 10^(−B·|Δλ|)`. Per-meteor magnitude follows the
population index `r`. Trail geometry radiates from the radiant great
circle, with length scaled by the geocentric velocity.

**References.**
- Koschack, R., Rendtel, J. 1990, WGN 18, 44 (visual flux model).
- Rendtel, J. et al. (annual), *IMO Meteor Shower Calendar*.
- McKinley, D. W. R. 1961, *Meteor Science and Engineering*.

**Implementation.**
- `crates/astronomy/src/meteors.rs`: the `MeteorShower` struct and the
  `IMO_WORKING_LIST` catalog (Quadrantids, Lyrids, η-Aquariids, Perseids,
  Orionids, Leonids, Geminids, Ursids), `solar_longitude_deg`,
  `zhr_at_solar_longitude` (Jenniskens profile), `observed_rate_per_hour`
  (Koschack-Rendtel inversion), `active_showers`, and `meteor_stream` — a
  deterministic SplitMix64-seeded Poisson sample of shower + sporadic
  meteors, time-binned by `(seed, jd_utc / window)` so the same session
  reproduces the same stream on every host.
- `crates/renderer/src/camera.rs`: a host-tier `MeteorLayer`
  (enabled / seed / rate_scale / window_seconds), `MAX_METEORS = 64`,
  CameraUniform `meteor_segments` + `meteor_params` rows (appended at the
  END of the uniform), and `meteor_uniforms()` mapping each streak through
  the shared apparent-direction transform.
- `crates/renderer/src/shaders/skyglow.wgsl`: a self-contained
  `meteor_radiance` evaluator (reusing the great-circle
  `satellite_streak_mask` helper) invoked from a single insertion point in
  the composition, so the meteor work stays isolated from the parallel
  `V-48` / `V-49` shader edits. One-frame streaks, no persistent train.
- The shower catalog is a transcribed-constant Rust table (citations in the
  module + `DATA_SOURCES.md`), so no committed data artifact / manifest row
  is needed.
- Session: `SessionMeteors` is appended to the scene with
  `#[serde(default, skip_serializing_if = …)]`, so existing sessions /
  presets stay byte-identical and the schema version is unchanged.

**Tests / validation.**
- Unit: `observed_rate_per_hour(ZHR=100, h_R=60°, r=2.2, lm=6.0)` ≈
  58.4 m/h (the Koschack-Rendtel value); recovers exactly ZHR at the
  zenith / 6.5-mag standard conditions; zero below the horizon.
- Unit: the Jenniskens activity profile peaks at maximum and decays
  symmetrically in solar longitude; solar longitude ≈ 140° at Perseid
  maximum.
- Unit / renderer: `meteor_stream` is deterministic for a fixed
  `(seed, time)` and differs across seeds; the packed renderer uniform
  carries unit-length streak endpoints, honours the `MAX_METEORS` cap, and
  is suppressed for external viewpoints.
- Session: an enabled meteor layer round-trips through the JSON schema.

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

### `V-50` Output colour management — ✅ done

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

**Implementation (as shipped).**
- `crates/renderer/src/colourspace.rs`: `OutputColourSpace { Srgb,
  DisplayP3, Rec2020 }` with the published CSS-Color-4 linear
  sRGB→target 3×3 gamut matrices (both D65, no chromatic adaptation),
  primary/white-point chromaticities for PNG tagging, and pinned tests
  (sRGB identity; a pure primary's chromaticity is preserved across the
  transform when re-projected through the target's own RGB→XYZ matrix;
  wide-gamut saturation check; string round-trip).
- `crates/renderer/src/tonemap.rs` + `shaders/tonemap.wgsl`: a
  `ColourManagementUniform` (gamut matrix rows padded to `vec4`) bound at
  tonemap binding 4. The shader applies the matrix as its final step
  after the Reinhard operator, clamped to non-negative. sRGB is the
  identity, so output is bit-identical to the pre-V-50 pipeline. The host
  swap-chain / PNG keeps the sRGB transfer function; only the primaries
  change, then the primaries are tagged on the output. Display-P3 (sRGB
  transfer) is exact; Rec.2020 is tagged with its primaries and uses the
  sRGB transfer as a documented approximation
  (`docs/standards-compliance.md`).
- `crates/common`: `OutputColourspaceArg` (clap `ValueEnum` + serde
  kebab-case) mirrors the engine enum; `SessionScene.output_colourspace`
  and the JSON `outputColourspace` field carry it. Session schema bumped
  to **v6**; the web frontend schema and committed preset sessions were
  regenerated under v6, and `data/manifest.toml` re-hashed.
- `apps/cli`: `--output-colourspace {srgb,display-p3,rec2020}`; the PNG
  encoder writes an `sRGB` chunk for sRGB and a `cHRM` primaries chunk
  for Display-P3 / Rec.2020 (switched from the `image` crate to `png`).
- `apps/viewer`: `--output-colourspace` flag plumbed onto the camera.
- `apps/web`: `StarView.set_output_colourspace`, a settings-panel
  dropdown, and localStorage / session persistence. The canvas remains
  sRGB-tagged, so wide-gamut primaries fall back to sRGB on unsupported
  browsers (documented).

**Hosts wired.** CLI / viewer / web.

---

## Solar system geometry — eclipses and occultations

### `V-51` Unified eclipse / occultation pass — ✅ (`V-51a` + `V-51b` + `V-51c` + `V-51d` + `V-51e` + `V-51f` shipped)

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
  attachments added. The star-sprite cull tracked by
  [`OccluderTarget::Stars`](crates/astronomy/src/occultation.rs)
  is now wired (V-51d) and consumed by the star vertex shader (one
  normalised dot + one `cos(radius)` compare per active occluder).
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
  ✅ Shipped. `astronomy::active_occluders` emits an always-on Moon →
  Stars cull entry (`OccluderTarget::Stars`, code `-1`) plus per-pair
  Moon → Planet entries (`OccluderTarget::Planet(i)`, code `2 + i`)
  whenever the Moon and a planet disk are in contact.
  `shaders/star.wgsl::vs_main` iterates the star-target entries (one
  normalised dot + one `cos(radius)` compare each) and hides the
  catalog sprite when inside the lunar disk; the existing analytic
  `disk_mask` keeps off-occultation frames bit-identical to the pre-
  V-51d render. Planning side: `find_lunar_occultation(observer,
  body, start, end)` with `LunarOccultedBody::{ Star { dir_date_eq },
  Planet(p) }` returns P1–P4 via the shared V-51a bisection refine.
- **`V-51e` Mercury / Venus transit of the Sun (planet → Sun).**
  ✅ Shipped. `astronomy::active_occluders` emits an
  `OccluderTarget::Sun` entry backed by Mercury or Venus whenever the
  inner planet is closer to the observer than the Sun *and* the
  apparent disks are in contact — the foreground-gate guard rejects
  the superior-conjunction near-alignment that would otherwise trip
  the pure-geometry classifier. The V-51b analytic-mask shader path
  iterates the new entry through the existing
  `occluder_subtract_mask(OCCLUDER_TARGET_SUN, …)` loop, drawing the
  planet's apparent disk as a black silhouette inside the solar
  sprite; the Koomen 1952 daylight falloff and Baumbach 1937 corona
  stay gated on `solar_eclipse_state` (Moon-on-Sun only) so a transit
  leaves the daylight sky untouched. Planning side:
  `find_planet_transit(observer, planet, start, end)` returns
  `PlanetTransitEvent` with `PlanetTransitEvent::is_interior()` and
  P1–P4 via the shared V-51a bisection refine. New `VenusTransit`
  scene preset (2012-06-06, Tokyo, az ≈ 113°, alt ≈ 55°, 2° FoV)
  frames the only transit in the validation canon until 2117.
- **`V-51f` Mutual planetary occultation (planet → planet).** ✅ Shipped.
  `active_occluders` now iterates every unordered planet pair, assigns
  the closer planet as the front disk, and emits one
  `OccluderTarget::Planet(back)` entry per pair currently in contact;
  the renderer's analytic-mask path (already generic over
  `OccluderTarget::Planet(i)` for `V-51d` Moon-on-Planet) subtracts the
  front disk from the back planet's source term without any shader
  churn. Planning side: `find_mutual_planetary_occultation(observer,
  planet_a, planet_b, start, end)` mirrors `find_lunar_occultation` —
  1-minute scan for the peak, shared `contact_times` bisection for
  P1–P4, and a `MutualPlanetaryOccultationEvent` carrying the front /
  back assignment, kind, peak obscuration, and contact times. Mutual
  planetary occultations are rare in practice (next visible event
  2065-11-22 Venus-Jupiter); a historical-event positive-detection
  test is deferred until the `L-06` DE440 upgrade lands, with current
  validation pinned to off-event `None`, same-planet rejection, and
  the producer contract that no Planet-on-Planet entries are emitted
  on a normal day.

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

### `V-52` Planetary rings and moons — ✅ done

**Item.** Today `V-35` renders Mercury–Neptune as bare disks. Three
visually unmistakable elements are missing: **Saturn's ring system**,
the **Galilean moons** (Io, Europa, Ganymede, Callisto), and **Titan**.
Without them, telescope-eyepiece scenes (`V-43`) look obviously wrong.

Following the `V-51a`–`V-51f` precedent, this item is split into
independently shippable sub-rungs so the astronomy and renderer work
can land in isolation:

| Sub | Scope | Status |
|---|---|---|
| `V-52a` | Saturn ring system (geometry, ring-plane shader, body-on-ring shadow) | ✅ done |
| `V-52b` | Galilean moons (Io / Europa / Ganymede / Callisto), Meeus-grade | ✅ done (upgraded to Lainey L1.2 by `V-52b-E5`) |
| `V-52b-E5` | Galilean moons precision upgrade — Lainey 2006 L1.2 (≤20″ / ±100 yr) | ✅ done (Lainey L1.2; pivot from Lieske 1998 E5) |
| `V-52c` | Titan, Meeus-grade | ✅ done |
| `V-52c-TASS17` | Titan precision upgrade — full TASS1.7 (~5″ / ±100 yr) | ✅ done |
| `V-52d` | Galilean shadow / occultation transits on Jupiter (reuses `V-51b`) | ✅ done |

**Deliberate non-goal scope.** No irregular moons of any planet, no
Neptunian / Uranian rings (faint, requires deep-field telescope sim),
no surface textures — the bodies stay as photometric point / disk
sources with magnitudes.

---

### `V-52a` Saturn ring system — ✅ done

**Item.** Saturn rendered with its A / B / C bands and the Cassini
Division, opened by the sub-Earth latitude `B`, and shadowed where the
planet body sits between Earth and the rear half of the ring plane.

**Scientific basis.**
- Ring-plane orientation: IAU WGCCRE 2015 Saturn pole `α₀ = 40.589°,
  δ₀ = 83.537°` in J2000 ICRS (slow century drift discarded — the ring
  opens through ±26.7° over a 29-year cycle, so a 0.01°/century pole
  drift is well below the test gate).
- Sub-Earth latitude `B` and sub-Sun latitude `B'` from Meeus 1998
  ch. 45: `sin B = sin i · cos β · sin(λ − Ω) − cos i · sin β` with
  `i = 28.075°, Ω = 169.508°` (J2000 ecliptic frame). `B'` uses the
  heliocentric ecliptic longitude / latitude of Saturn.
- Ring inner / outer radii from the Cassini orbital fits (Porco et al.
  2005): C inner 74 510 km, B inner 91 980 km, B outer 117 580 km,
  Cassini outer 122 050 km, A outer 136 775 km; Saturn equatorial
  radius 60 268 km.
- Band brightness ratios (Dones et al. 1993, geometric albedo at
  B ≈ 26°): A = 0.50, B = 1.00 (anchor), Cassini = 0.15, C = 0.20.
- Planet shadow on the rings: ring pixels behind Saturn (positive
  line-of-sight depth) and inside the body silhouette darken to zero;
  the ring opening sets which annulus is occluded.

**References.**
- Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 45 ("Ephemeris
  for Physical Observations of Saturn's rings").
- Porco, C. C. et al. 2005, Science 307, 1226 (Cassini ring geometry).
- Dones, L. et al. 1993, Icarus 105, 184 (ring photometric profiles).
- Archinal, B. A. et al. 2018, CMDA 130, 22 (IAU WGCCRE 2015 pole).

**Implementation.**
- `crates/astronomy/src/ephemeris.rs`: `SaturnRingApparent { B, Bp,
  ring_pole_eq, position_angle_rad }` + `apparent_saturn_ring(jd)` and
  `apparent_saturn_ring_topocentric(observer)`.
- `crates/renderer/src/camera.rs`: extends the planet uniform with a
  `saturn_ring` block (ring-pole eq vec3, `sin|B|`, illumination-side
  sign, four ring radii in radians). Inactive when planets are off or
  Saturn is below the horizon.
- `crates/renderer/src/shaders/skyglow.wgsl`: `saturn_ring_radiance`
  composes the elliptical annulus, evaluates band brightness, and
  subtracts the body-on-ring shadow.
- Validation gallery: `saturn-eyepiece` preset added.

**Tests / validation.**
- Unit: `B` matches Meeus tabulated openings within 0.3° at the
  1995-08-10 (edge-on), 2002-12-17 (south-face maximum, B ≈ −26.7°),
  2009-09-04 (edge-on) and 2017-05-28 (north-face maximum, B ≈ +26.7°)
  reference epochs.
- Unit: ring-pole equatorial direction at J2000 is within 0.1° of the
  IAU pole.
- Unit: topocentric and geocentric ring API return identical orientation
  at the V-52a accuracy budget (Earth-radius parallax cannot move the
  ring orientation).

**Follow-up.** A `saturn-eyepiece` deterministic scene preset (Saturn
framed near the 2017 northern-face solstice in eyepiece mode) is left as
a separate small PR so the preset JSON export pipeline and its
round-trip tests can be bumped together.

**Hosts wired.** CLI / viewer / web.

---

### `V-52b` Galilean moons — ✅ done (Meeus-grade shipped; upgraded to Lainey L1.2 by `V-52b-E5`)

**Item.** Io, Europa, Ganymede, Callisto as point sources next to
Jupiter. Shipped via the Meeus 1998 ch. 44 simplification of
Lieske's E5 theory (the same algorithm family the `astro` crate's
`apprnt_rect_coords` is built around); reaches roughly arcminute-grade
accuracy at the edges of the ROADMAP ±100-yr budget — enough for
naked-eye / small-eyepiece identification, **not enough** for the full
~5″ precision gate. The latter is tracked as `V-52b-E5`.

**Scientific basis.** Meeus 1998 *Astronomical Algorithms* ch. 44
("Positions of the Satellites of Jupiter"), itself a low-precision
reduction of Lieske 1977 / Lieske 1998 E5 theory.

**References.**
- Meeus, J. 1998, *Astronomical Algorithms*, 2nd ed., ch. 44.
- Lieske, J. H. 1977, A&A 56, 333 (E2 theory, foundation of E5).
- Archinal, B. A. et al. 2018, CMDA 130, 22 (Galilean radii & rotation).

**Implementation.**
- `crates/astronomy/src/moons.rs` (new): `GalileanMoon` enum,
  `GalileanMoonApparent { RA, Dec, distance_au, angular_radius_rad,
  magnitude }`, `apparent_galilean_moons{,_topocentric}` API mirroring
  the planet / Saturn-ring shape. Magnitudes use Meeus 1998 table 41.A
  reduced `V(1,0)` values plus the standard `5·log10(r·Δ)` distance
  term where `r`, `Δ` are Jupiter's heliocentric and geocentric
  distances in AU.
- `crates/renderer/src/camera.rs`: extends the planet uniform with a
  `galilean_eq_radius[4]`, `galilean_rgb_magnitude[4]`, `galilean_params`
  block. Gated by the same `planets_enabled` flag so Galilean moons
  share one host control with Jupiter itself.
- `crates/renderer/src/shaders/skyglow.wgsl`: new
  `galilean_disk_radiance` evaluator routes the four moons through the
  existing point-light + magnitude-to-flux pipeline.
- `crates/renderer/src/text.rs`: `GALILEAN_LABELS` registered behind
  the existing planet-labels overlay layer.

**Tests / validation (Meeus-grade).**
- Unit: `GalileanMoon::ALL` ordering matches `[Io, Europa, Ganymede,
  Callisto]` (canonical Lieske ordering).
- Unit: every moon stays within its maximum tabulated elongation from
  Jupiter at J2000.
- Unit: pairwise angular separations are > 1″ at J2000 (none of the
  four collapse onto each other in the renderer-visible projection).
- Unit: apparent V magnitudes near the 2000 Jupiter opposition land
  within 0.4 mag of the tabulated values `[5.0, 5.3, 4.6, 5.7]`.
- Unit: Io's sky-plane offset reverses across half its 1.77-day orbital
  period.
- Unit: topocentric and geocentric APIs agree to within ≈10″
  (Earth-radius parallax bound at Jupiter's mean distance).

**Deliberately out of scope for this rung.**
- `~5″ / ±100-yr` accuracy gate against JPL Horizons — tracked by
  `V-52b-E5`.
- `jupiter-eyepiece` validation-gallery scene preset — deferred to a
  follow-up PR so the preset JSON-export pipeline and its round-trip
  tests bump together, mirroring the `saturn-eyepiece` deferral from
  `V-52a`.
- Shadow / occultation transits on the Jovian disk — `V-52d` consumes
  the geometry produced here and re-uses the `V-51b` occluder array.

**Hosts wired.** CLI / viewer / web (all driven by the shared
`planets_enabled` flag; no new host knob).

---

### `V-52b-E5` Galilean moons — Lainey 2006 L1.2 precision upgrade — ✅ done (pivoted from Lieske 1998 E5)

**Item.** Replaced the Meeus-grade Galilean-moon backend from `V-52b`
with the full Lainey, Duriez & Vienne 2006 L1.2 semi-analytic series
so apparent Jovicentric positions stay within ≈20″ of JPL Horizons
over the ROADMAP ±100-yr budget — a >10× tightening of the previous
Meeus-grade 200″ bound, with the worst-case Callisto out-of-plane drift
(≈180″ at the ±100-yr edge) eliminated outright.

**Pivot from Lieske 1998 E5.** This rung originally targeted the Lieske
1998 E5 trigonometric series (A&AS 129, 205). The published E5
coefficient tables are no longer reachable from a reproducible sandbox
(A&A `ds7367` PDF returns 404, IMCCE FTP only hosts Lainey's L1.x
family, Lieske's `galsat` Fortran mirror at cococubed.com is dead). We
pivoted to Lainey 2006 L1.2, which is the modern successor to Lieske
E5: same accuracy class (≤5″/100 yr against the underlying numerical
integration), same public API, reachable Fortran + coefficient files
at `ftp://ftp.imcce.fr/pub/ephem/satel/galilean/L1/L1.2/`.

**Scientific basis.** Lainey 2006 L1.2 — a semi-analytic representation
of the IMCCE Galilean-satellite numerical integration `Galsat`, fitted
to all 1891–2003 ground-based and Galileo / Cassini space observations.
Elements per moon: a (semi-major axis), L (mean longitude), z = e·
exp(iϖ) (complex eccentricity), ζ = sin(i/2)·exp(iΩ) (complex inclin-
ation). Up to ≈160 trigonometric terms per moon plus a degree-8
Chebyshev correction over the validity window [J1140, J2760]. The
L1.2 orbital elements are converted to Cartesian via the IMCCE
`ELEM2PV` Kepler-iteration kernel and rotated into the J2000 mean
equator/equinox frame.

**References.**
- Lainey, V., Duriez, L., Vienne, A. 2006, A&A 456, 783 —
  *Synthetic representation of the galilean satellites orbital
  motions from L1 ephemerides* (the L1.2 publication).
- IMCCE 2006, *L1.2 distribution*,
  `ftp://ftp.imcce.fr/pub/ephem/satel/galilean/L1/L1.2/` — Fortran
  source `L1.2.f`, coefficient files `GalileanL1.2.dat` /
  `BisL1.2.dat`, validation `TestL1.2.res`.
- Lieske, J. H. 1998, A&AS 129, 205 — the original E5 theory the
  rung originally targeted; kept for citation completeness.
- JPL Horizons On-Line Ephemeris System
  (https://ssd.jpl.nasa.gov/horizons/) — the empirical reference the
  accuracy gate is anchored against.

**Implementation areas.**
- `crates/astronomy/src/moons/lainey_l1.rs` — Fortran-faithful Rust
  port of the IMCCE `DL1_2` evaluator: parses the embedded
  `BisL1.2.dat` once on first use, evaluates the trigonometric
  series + Chebyshev correction, runs the `ELEM2PV` Kepler iteration,
  and rotates the result into the J2000 mean equator/equinox frame.
  Returns `JovicentricState { position_km, velocity_km_s }` in the
  observer-facing J2000 frame.
- `crates/astronomy/src/moons.rs` — the `lieske_e5` substitution
  point is renamed to `lainey_l1` and the caller now adds the moon's
  3D L1.2 position directly to Jupiter's km position (no sky-plane
  projection round-trip).
- `crates/astronomy/data/BisL1.2.dat` — the IMCCE coefficient table,
  84 384 bytes, embedded via `include_str!` and pinned in
  `data/manifest.toml` as `lainey-2006-l12-galilean-coeffs`.

**Tests / validation.**
- `moons::tests::galilean_matches_horizons_within_l1_budget` — enforces
  the per-moon Horizons residual < 20″ at every fixture epoch. Measured
  residuals (max per moon × epoch): 14.3″ (Io 1900), 8.9″ (Ganymede
  1900), 15.8″ (Callisto 1900); ≤7.1″ at 2000; ≤5.5″ at 2100.
- `moons::lainey_l1::tests::parser_recovers_per_moon_term_counts` —
  pins the per-(satellite, element) term counts of `BisL1.2.dat` so
  the embedded file cannot drift unnoticed.
- `moons::lainey_l1::tests::jovicentric_state_returns_finite_values_at_j2000`
  — sanity bounds on the J2000 output.

**Hosts wired.** CLI / viewer / web (all consume `apparent_galilean_
moons{,_topocentric}` so the L1.2 upgrade is transparent).

**Known follow-ups.**
- The remaining ~10″ residual at the 1900 epoch is dominated by
  Earth-Jupiter vector reduction differences (Horizons uses
  DE441 / IAU 2006 precession; L1.2 was fitted against DE406).
  Aligning the reduction is the natural follow-up to drive the
  bound below 5″.
- The V-52d shadow producer (`crates/astronomy/src/jupiter_shadows.rs`)
  still uses the Meeus ch. 44 truncation directly. After this rung,
  V-52d shadow positions disagree with V-52b moon positions by the
  full L1.2-vs-Meeus residual (≈180″ ≈ 8 R_J on Callisto). The
  consistency test `earth_xy_matches_apparent_galilean_moons_at_j2000`
  is marked `#[ignore]` until a follow-up rung **`V-52d-L1.2`** ports
  the shadow projection onto Lainey L1.2 too.

---

### `V-52c` Titan — ✅ done (Meeus-grade shipped; upgraded to full TASS1.7 by `V-52c-TASS17`)

**Item.** Titan as a point source ≈3′ from Saturn (Saturn's brightest
moon, V ≈ 8.4, easily reachable in a small telescope).

**Status.** Shipped at Meeus 1998 ch. 45 accuracy (the simplification of
the TASS theory of Vienne & Duriez 1995 that the `astro` crate already
implements). Same accuracy posture as the V-52b Meeus-grade Galilean
moons: good for naked-eye / small-eyepiece identification within a few
arcseconds near J2000, drifting to ≈10–60″ over the ROADMAP ±100-yr
budget. The full ~5″ / ±100-yr TASS1.7 precision upgrade is tracked
separately as the follow-on rung `V-52c-TASS17`.

**Scientific basis (current).** Meeus 1998 *Astronomical Algorithms*
ch. 45 simplified analytic theory of Saturn's eight moons, restricted
here to Titan. The published reduction is itself based on Vienne &
Duriez 1995 (the same theory `V-52c-TASS17` will upgrade to full
precision).

**Scientific basis (target rung `V-52c-TASS17`).** Full TASS1.7
(Vienne & Duriez 1995) coefficients, transcribed from a verified
machine-readable source distributed by IMCCE.

**References.**
- Meeus, J. 1998, *Astronomical Algorithms* (2nd ed.), ch. 45.
- Vienne, A., Duriez, L. 1995, A&A 297, 588 (TASS1.7 Saturnian moons).
- Karkoschka, E. 1998, *Icarus* 133, 134 (Titan visual photometry,
  source of the `V(1, 0) = −1.28` reduced magnitude).
- Archinal, B. A. et al. 2018, CMDA 130, 22 (Titan physical radius
  2575 km from IAU WGCCRE 2015).

**Implementation scope.** `crates/astronomy/src/moons.rs` gains a
`TitanApparent` value, `apparent_titan(jd)`, and
`apparent_titan_topocentric(observer)`, mirroring the V-52b Galilean
API one-for-one. The renderer publishes a single-slot `titan_*` uniform
block next to the V-52b `galilean_*` block in both `skyglow.wgsl` and
`star.wgsl` (Pod / Zeroable layout, same shape and gating as the
Galilean block). Text labels reuse the planet-labels overlay toggle.
Gated by the existing `planets_enabled` host flag — no new session-
schema field, no new UI control, no new WASM setter.

**Tests / validation.** `crates/astronomy/src/moons.rs::tests` pins:

- maximum elongation of Titan from Saturn (< 3.4′ across one full
  Titonian period);
- full-period sky swing (> 200″ across half a period of 15.95 d);
- magnitude near 2003-12 Saturn opposition within 0.4 mag of the
  published V ≈ 8.3;
- sub-arcsecond apparent disk radius across a full period;
- topocentric vs geocentric parallax bounded by ≈5″ at J2000;
- direction unit-vector renormalisation;
- roadmap headline configuration: Titan separation from Saturn at
  J2000 inside (0.1′, 3.5′).

The renderer pins the Pod uniform plumbing
(`renderer::camera::tests::titan_uniform_matches_apparent_titan_at_j2000`
and `titan_uniform_disabled_when_planets_off`). Sub-5″ JPL-Horizons
agreement at multiple epochs across ±100 yr is the gate the follow-on
`V-52c-TASS17` rung will pin.

**Hosts wired.** CLI / viewer / web — through the same
`planets_enabled` toggle V-52b uses (CLI `--no-planets`, viewer toggle,
web `set_planets_enabled`).

---

### `V-52c-TASS17` Titan — full TASS1.7 precision upgrade — ✅ done

**Item.** Replace the Meeus-grade Titan backend from `V-52c` with the
full TASS1.7 (Vienne & Duriez 1995) Titan series, transcribed from a
verified machine-readable IMCCE distribution, and pin a multi-epoch
Horizons-anchored ~5″ / ±100-yr validation matrix.

**Status.** Done. `crates/astronomy/src/moons/tass17.rs` now evaluates
the full TASS1.7 series — a faithful Rust port of the IMCCE `tass17.f`
subroutines `CALCLON` / `CALCELEM` / `EDERED` / `LECSER` — against the
vendored series table `crates/astronomy/data/redtass7.dat` (extracted
from `tass17.f` by `scripts/build-tass17.sh`, manifest id
`vienne-duriez-1995-tass17-titan-coeffs`). The public entry point is
`tass17::kronocentric_state_j2000(jd)`, returning Titan's 3D
Saturn-centred position + velocity in the J2000 mean equator / equinox
frame; `moons.rs::titan_from_saturn` adds that vector directly to
Saturn's apparent position (with one parent-planet light-time
retardation step), exactly like the Galilean L1.2 path. Hyperion
(TASS index 7) is excluded from the vendored series because `CALCLON`
fixes its proper longitude `DLO(7) = 0`, leaving the Titan result
unchanged.

The port is validated bit-for-bit against the IMCCE `EXAMP7.res`
reference positions (<1e-10 AU,
`tass17::tests::matches_imcce_examp7_reference`), and the apparent
Titan-vs-Saturn offset matches `data/horizons_titan.csv` to ≈0.1″ at
J2000 and ≈3–4″ at the ±100-yr extremes — inside the ~5″ bar
(`moons::tests::titan_matches_horizons_within_tass17_budget`,
`TASS17_MAX_OFFSET_ERR_ARCSEC = 5.0`). The residual at the extremes is
dominated by Saturn's own VSOP87 ephemeris (the `astro` crate) and the
fixture's 0.01ˢ/0.1″ quantization, not the TASS1.7 model.

**Scientific basis.** TASS1.7 (Vienne & Duriez 1995, A&A 297, 588)
restricted to Titan, with the full coefficient tables rather than the
Meeus 1998 ch. 45 truncation.

**References.**
- Vienne, A., Duriez, L. 1995, A&A 297, 588 (TASS1.7).
- IMCCE 1996, TASS1.7 distribution, `ftp://ftp.imcce.fr/pub/ephem/
  satel/tass17/` — Fortran source `tass17.f` (subroutines + embedded
  series) and reference positions `EXAMP7.res`.

**Implementation scope.** Replaced the `astro` crate's Saturnian-moon
backend in `apparent_titan{,_topocentric}` with the TASS1.7 driver in
`crates/astronomy/src/moons/tass17.rs`. The only API change is internal
to the module (`titan_offset` → `kronocentric_state_j2000`, a 3D state
mirroring the Galilean `lainey_l1::jovicentric_state_j2000`); the
public `TitanApparent` surface and all V-52c-shipped tests are
unchanged.

**Tests / validation.** IMCCE `EXAMP7.res` bit-level golden values at
three epochs; JPL Horizons agreement within 5″ at 1900 / 2000 / 2100;
parsed series term counts pinned against `redtass7.dat`; plausible
distance and orbital-speed sanity bounds.

**Hosts wired.** Already CLI / viewer / web through `V-52c` — this
rung is a pure backend precision upgrade, leaving the renderer /
shader / label pipelines untouched.

---

### `V-52d` Galilean shadow / occultation transits on Jupiter — ✅ done

**Item.** Reuses the `V-51b` analytic-mask occluder array to draw the
shadows of Io / Europa / Ganymede / Callisto crossing the Jovian disk,
and to occult each moon when it passes behind Jupiter (moon ↔ moon
mutual occultation is deferred — see follow-up below).

**Dependencies.** Requires `V-52b` for the Jovicentric geometry. Reuses
`V-51a` (occultation primitives) and `V-51b` (analytic-mask occluder
uniform array).

**Implementation.**
- `crates/astronomy/src/jupiter_shadows.rs` (new): exposes the 3D
  Jovicentric rectangular coordinates of each Galilean moon — once
  from the Earth's line of sight and once from the Sun's — using the
  Meeus 1998 ch. 44 truncated series. Predicates
  `shadow_on_jupiter`, `moon_in_front_of_jupiter`, and
  `moon_behind_jupiter` close the geometry; `galilean_shadow_disks_at`
  returns the ready-to-pack analytic disks for the V-51b occluder
  array, with shadow radius = `moon.radius_km / Δ_Jupiter` (the
  silhouette spans the same physical extent on Jupiter as the moon
  itself).
- `crates/astronomy/src/planning.rs`: `active_occluders` emits one
  `OccluderTarget::Planet(3)` (= Jupiter) entry per active shadow,
  reusing the V-51d / V-51e / V-51f pipeline.
- `crates/renderer/src/camera.rs`: the V-52b Galilean uniform now
  packs a negative-radius sentinel for moons currently behind Jupiter
  from the observer; `shaders/skyglow.wgsl`'s `galilean_disk_radiance`
  skips those sprites so a moon disappears while it sits inside
  Jupiter's silhouette.
- Scene preset: `jupiter-shadow-transit` (`docs/presets/sessions/jupiter-shadow-transit.json`),
  pinned to the 2008-12-20 14:00 UT Io transit from Roque de los
  Muchachos.

**Tests / validation.** Shadow transit ingress times at known epochs
within 5 minutes of JPL Horizons; one deterministic eyepiece render of
a Galilean shadow-transit configuration.
- Pinned ingress: 2008-12-20 13:14 UT Io shadow transit, geocentric
  (PHEMU09 reference), within 5 min
  (`jupiter_shadows::tests::io_shadow_ingress_within_five_minutes_of_horizons_2008_12_20`).
- Producer contract: V-52d shadow disk appears in `active_occluders`
  exactly when the geometry says so, and only at the moon's
  silhouette extent
  (`planning::tests::active_occluders_emit_io_shadow_at_2008_12_20_transit`,
  `planning::tests::active_occluders_emit_no_galilean_shadow_off_event`).
- Renderer uniform contract: the analytic-mask uniform carries a
  Planet(Jupiter)-targeted entry at the 2008-12-20 14:00 UT epoch,
  with the right kind code, unit-length direction, and a radius
  inside the Io-silhouette range
  (`camera::tests::occluder_uniform_emits_io_shadow_at_2008_12_20_transit`).

**Follow-up.** Moon ↔ moon mutual occultation (a moon hiding another
moon) is intentionally deferred. The V-51b `OccluderTarget` enum
reserves codes only for the Sun, the Moon, the seven planets, and the
star cull; encoding the four Galilean moons individually would
require either a new target enum or a per-moon analytic-mask
extension in `shaders/skyglow.wgsl`. PHEMU-cadence moon ↔ moon events
are rare (≲ once per year, mostly outside opposition) so the deferral
keeps the V-52d shadow / behind-Jupiter scope clean.

**Hosts wired.** CLI / viewer / web.

---

### `V-53` Resolved star clusters — ✅ done (showpiece bootstrap slice)

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

**Status (showpiece bootstrap shipped).**
- `crates/catalog/data/cluster_membership.csv` (new) carries a
  hand-curated membership table for 4 clusters / 34 stars: Pleiades
  (M45, 9 named members), Praesepe / Beehive (M44, 11 bright core
  members), Double Cluster (NGC 869 + NGC 884, HYG-resolvable bright
  members). Hyades (Mel 25) is intentionally deferred — it has no
  current V-42 DSO marker to suppress and will land with the
  Cantat-Gaudin upgrade.
- `crates/catalog/src/clusters.rs` (new) parses the CSV via
  `OnceLock` and exposes
  `cluster_members(DeepSkyId) -> &'static [ClusterMember]` and
  `is_resolved_as_member_field(DeepSkyId) -> bool`.
- `crates/catalog/src/deepsky.rs` extends `DeepSkyCatalog` with the
  documented `resolve_as_member_field(DeepSkyId) -> bool` default
  method; both `MessierCatalog` and `NgcBrightCatalog` consult the
  cluster module.
- `crates/renderer/src/overlay.rs` skips marker geometry for any
  resolve-as-member-field DSO; the label pass is untouched so the
  cluster label still sits over the resolved star field.
- `data/manifest.toml` carries the new
  `open-cluster-membership-bootstrap` row with DOI, license, and the
  `scripts/extract-cluster-membership.py` regeneration command.
- `scripts/extract-cluster-membership.py` (new) reproduces the CSV
  byte-identically from a Python bootstrap list; the
  `--from-cantat-gaudin` switch is stubbed and gated for the
  follow-up extraction.

**Tests / validation.**
- `pleiades_named_seven_positions_match_within_one_arcminute` resolves
  HYG positions for the 7 named bright Pleiades stars and asserts each
  is within 1' of the SIMBAD / Hipparcos reference — the V-53 gate.
- `deep_sky_markers_suppress_v53_resolved_clusters` asserts the
  renderer drops marker geometry for M45 / M44 / NGC 869 / NGC 884 and
  keeps it for unrelated DSOs (M31, NGC 7000).
- `pleiades_named_seven_are_members`,
  `praesepe_is_resolved_as_member_field`,
  `double_cluster_is_resolved_as_member_field`,
  `unrelated_dso_is_not_resolved_as_member_field`,
  `resolved_cluster_ids_match_v53_scope`,
  `cluster_member_hyg_ids_are_unique_per_cluster` pin the membership
  table.

**Deliberate non-goal scope.** No globular-cluster star-by-star
resolution (too dense, no per-member catalog at hobbyist scale — keep
as DSO disk). No cluster colour-magnitude diagrams.

**Follow-up.** Replace the hand-curated bootstrap with a deterministic
Cantat-Gaudin 2020 extraction via VizieR
(`scripts/extract-cluster-membership.py --from-cantat-gaudin`,
currently a stub). The deeper Double Cluster core (V > 9) and Hyades
will follow when that path lands.

**Hosts wired.** CLI / viewer / web (catalog seam — no host-side knob).

---

### `V-54` Double / binary star resolution — ✅ done

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

**Implementation.**
- `crates/catalog/data/double_stars.csv` (new): WDS-derived showpiece
  table keyed by the merged HYG primary `id` with `(ρ, θ, m1, m2, B−V1,
  B−V2, epoch, WDS id)`. Regenerated by `scripts/extract-double-stars.py`
  and pinned in `data/manifest.toml` (`wds-double-stars-bootstrap`).
- `crates/catalog/src/doubles.rs` (new): parses the table and
  `resolve_doubles(Vec<Star>) -> Vec<Star>` replaces each matching merged
  primary with two component `Star`s — primary at the catalog position,
  secondary offset along the exact great-circle step at `(ρ, θ)`, each
  carrying its own B−V through the `V-23` `bv_to_rgb` pipeline.
- `crates/catalog/src/catalog.rs`: both load paths (`load_from_csv` and
  the embedded `load_from_binary`) run `resolve_doubles`, so CLI / viewer
  (HYG CSV) and web (embedded binary) all resolve the same pairs with no
  host-specific code. Matching is by HYG `id` when present, else by
  position (15″ tolerance) since the embedded binary drops identifiers.
- Bootstrap scope: Algieba (γ Leo, one merged HYG row) and the ε Lyrae
  "Double Double" (HYG ids 91633 / 91639, each itself an unresolved pair
  → four sprites). Mizar is **already** two HYG rows (A = 65173, B =
  118887 at ~19″, with Alcor = 65272 at ~12′), as are Albireo and Castor,
  so they are deliberately excluded to avoid a phantom third component.
- Acceptance threshold: the native hosts build the GPU instance buffer
  once and only vary FOV per frame, so both components are always emitted
  and the "merged below 1 px" behaviour is an emergent property of the
  renderer's *linear* HDR PSF accumulation (`V-16` / `V-17`): two
  sub-pixel components of flux `f1`, `f2` sum to one PSF of flux
  `f1 + f2`, which a pinned test confirms is within ~0.1 mag of the
  original merged magnitude (no brightening, no aliasing).

**Tests / validation.** `crates/catalog/src/doubles.rs` unit tests:
Algieba splits into two components at the WDS separation (4.6″) and
position angle (126°) by both id and position match; the ε Lyrae pair
resolves into four sprites; combined component flux stays within 0.15 mag
of the merged HYG magnitude; Albireo's existing HYG rows render a
gold/blue pair through `V-23`; the already-resolved Mizar B row is not
re-split; ordinary stars pass through unchanged.

**Deliberate non-goal scope.** No spectroscopic-binary modelling, no
orbital animation for short-period visual binaries (the catalog epoch
position is used as-is).

**Hosts wired.** CLI / viewer / web.

---

## Earth orbit

### `V-55` Artificial satellites (TLE / SGP4) — ✅ done

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

**Implementation (shipped).**
- `crates/astronomy/src/satellites.rs`: TLE parser + SGP4 propagation
  via the vendored pure-Rust `sgp4` crate (Vallado et al. 2006), returning
  geocentric TEME position. Topocentric reduction reuses
  `observer_equatorial_position_km` (shared GMST rotation) so the TEME
  position and the WGS84 observer share one frame; RA/Dec + local sidereal
  time give alt/az and slant range. A conical umbra/penumbra Earth-shadow
  test (built from the apparent Sun) gives the sunlit fraction, and the
  McCants/QuickSat standard-magnitude convention gives apparent magnitude.
- `crates/renderer`: a `SatelliteLayer` on `Camera` plus a per-frame
  satellite uniform block (direction + magnitude, streak endpoint +
  visibility flag, count/enabled/exposure header). `shaders/skyglow.wgsl`
  renders each sunlit, above-horizon satellite as a neutral point sprite,
  or a great-circle motion streak when the exposure field is positive.
- `crates/common`: a manifest-pinned curated TLE snapshot
  (`data/satellites/curated_tle.txt`: ISS, HST, NOAA-20, a Starlink, and
  geostationary GOES-16), embedded via `include_str!`; session schema v6
  adds the `satellites` block; an `iss-pass` scene preset (dark-sky
  near-zenith ISS pass) exercises the layer.
- Hosts: CLI `--satellites` / `--satellite-exposure-seconds`; viewer `L`
  key toggle (+ same flags); web `StarView.set_satellites` WASM binding and
  a settings-panel toggle / exposure control with session + URL
  round-trip. Live TLE fetch is opt-in only (see
  `docs/standards-compliance.md`); the default render path uses the pinned
  snapshot for deterministic renders.

**Tests / validation (shipped).** `astronomy::satellites` unit tests pin
the SGP4 position against the AIAA 2006-6753 catalog-88888 reference
vector (sub-km), the conical Earth-shadow umbra/penumbra classification,
and the standard-magnitude reference geometry; the renderer pins the
satellite uniform packing for a visible ISS pass; `make manifest-check`
pins the TLE snapshot bytes.

---

## Interactive UX

### `V-56` Object search, GoTo, and info panel — ✅ done

**Status.** Shipped across all three hosts. The `apps/web` host has the
search box, ranked dropdown, click-to-`goto` slew, and apparent-state
info panel. The CLI exposes `--goto <name>`, which resolves a query
through the shared resolver, centres the local alt-az view on the
target, and prints an info summary before rendering. The desktop
viewer adds an interactive `/`-triggered search prompt (typed into the
window title bar, Enter slews + shows the target's info summary, Esc
cancels) plus a startup `--goto` flag mirroring the CLI. The engine-side
resolver (`crates/common/src/goto.rs`, `resolve_goto_query` /
`resolve_goto_id`) is shared by the CLI and viewer and keeps the
solar-system magnitude / distance conventions identical to `apps/web`.
The CPU-side `crates/catalog/src/search.rs` index covers ~1.2k bright
named stars, the 110 Messier objects, the bright NGC / IC subset, and
the nine solar-system bodies (Sun + Moon + planets minus Earth) with
Japanese aliases. Click / hover *picking* of an arbitrary rendered body
remains a web-only affordance (the CLI is non-interactive and the
viewer reaches the same objects through the search prompt); the
`crates/renderer/src/picking.rs` R32Uint pick buffer and the
`SelectedTarget` session field described below stay open as a web
refinement and are not required for native-host parity.

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
- Unit (`crates/common/src/goto.rs`): `resolve_goto_query` ranks
  `"Vega"`, `"M31"`, `"Saturn"`, and `"土星"` to the expected ids,
  produces a finite centred local view, errors on empty / unknown
  queries, and the RA/Dec summary formatting is pinned at its bounds.
- Unit (`apps/cli`): `--goto` flag parsing (none / single / multi-word
  designation) plus the end-to-end parse → resolve → centred-view path.
- Viewer parity is covered by the shared resolver unit tests plus the
  CLI ↔ viewer parity matrix in `docs/scene-presets.md`.
- Visual: a deterministic CLI render with `--goto m31` from Tokyo can be
  added to the gallery as a follow-up.

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

### `L-09` Observation-planning polish — ✅ done

**Item.** Favourites, "tonight's recommended objects", Moon-impact score,
visibility score, and optional calendar export, all derived from
documented planning helpers.

**Scientific basis.** Moon-impact score follows Krisciunas-Schaefer 1991
moonlight sky-brightness contribution at the target's altitude. Visibility
score combines altitude-vs-time, twilight bands, and (when present from
`V-39`) site sky brightness.

**Implementation.**
- `crates/astronomy/src/planning.rs`: the Krisciunas-Schaefer 1991
  moonlight model — `moon_sky_brightness_nanolamberts` (Eq. 15) built
  from the illuminance (Eq. 20), scattering function `f(ρ)`
  (Eqs. 16–18), and relative airmass (Eq. 3), with
  `nanolamberts_from_v_mag` / `v_mag_from_nanolamberts` (Eq. 27) for the
  V-band surface-brightness conversion. `moon_impact` evaluates ΔV for a
  fixed-equatorial target against a Moon-free baseline; `visibility_score`
  composes an altitude × dark-window × Moon-clarity score over the
  evening window; `rank_targets` / `planning_targets_from_bodies` build
  "tonight's recommended objects"; `icalendar_for_targets` emits an
  RFC 5545 `.ics` document of each target's observable dark window.
  The Moon-free baseline reads the `V-39` light-pollution zenith
  brightness (`LightPollution::zenith_sqm_mag_per_arcsec2`).
- `apps/cli`: `--plan-json` prints the ranked plan + per-target scores to
  stdout and `--plan-ical <path>` writes the calendar; both exit before
  the GPU render path.
- `apps/web` + `apps/web/frontend`: the `StarView` bridge gains
  `planning_recommended_json` and `planning_ical`; the Planning settings
  card shows the recommended ranking with visibility score, max altitude,
  and Moon ΔV, a localStorage-backed favourites star toggle, and an
  "Export .ics" download button.

**Tests / validation.**
- Unit: Moon-impact for a target at 20° altitude with the Moon at 60°
  altitude and 90% illumination matches the K-S-derived ΔV ≈ 2.99
  mag/arcsec² (`moon_impact_matches_krisciunas_schaefer_reference`).
- Unit: monotonicity in lunar phase / Moon-target separation, V-mag ↔
  nanolambert round-trip + pinned 21.6 mag anchor, visibility scores in
  `[0, 1]` and descending after ranking, iCalendar well-formedness, and a
  pinned JD → UTC timestamp.

**Hosts wired.** Web (primary UX); CLI exports JSON and iCalendar of the
same calculations.

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

### `L-14` Public demo gallery — ✅ done

**Item.** Curated, narrated public demo page at
[`docs/demo-gallery.md`](docs/demo-gallery.md) (and
[`docs/demo-gallery.ja.md`](docs/demo-gallery.ja.md)) covering 12 of the
most visually-striking deterministic scene presets: total solar eclipse,
Belt of Venus, Galilean shadow on Jupiter, Bortle 1 ↔ Bortle 8 light-
pollution comparison, full-sky Mollweide, galactic-north external
viewpoint, and the canonical Tokyo / dark-sky / sunset / moonlit-night
baselines.

**Scientific basis.** Demo discipline: each scene is reproducible and
cites the chosen catalog, atmosphere, and ephemeris versions through the
session schema. The committed PNGs are hashed in
`data/manifest.toml` under `kind = "generated"` with
`preprocessing = "scripts/render-demo-gallery.sh"`, so `make manifest-check`
(part of `make ci`) fails on silent byte drift.

**Implementation.**
- `docs/demo-gallery.md` / `docs/demo-gallery.ja.md` — curated narrated
  index with the preset name on every entry.
- `scripts/render-demo-gallery.sh` and `make demo-gallery` /
  `make demo-gallery-check` — mirrors the validation-gallery script
  layout but renders only the curated subset.
- `docs/assets/demo-gallery/*.png` — 480 × 270 PNGs, one per curated
  preset, each manifest-tracked.
- README front-door section (English + Japanese) with a 3-up thumbnail
  strip and a `make demo-gallery` callout.

**Tests / validation.** `make manifest-check` (in `make ci`) re-hashes
the committed bytes; `make demo-gallery-check` is the opt-in exact
screenshot regression for pinned-GPU CI environments.

**Hosts wired.** Documentation + CLI re-render. The optional in-app
gallery page over `apps/web/frontend/` is deliberately deferred — the
Markdown-first surface ships from one source-of-truth subset that the
web UI can later read.

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

### `L-19` SIMBAD / VizieR deep links — ✅ done

**Item.** Select a star → external link with the right SIMBAD / VizieR
query. External services stay optional and out of deterministic renders
(no network call from the rendering pipeline).

**Scientific basis.** SIMBAD / VizieR are the canonical CDS lookup
services for stellar metadata; their query URL formats are documented
(Wenger 2000, A&AS 143, 9; Ochsenbein 2000, A&AS 143, 23).

**Implementation.**
- `crates/catalog/src/links.rs`: `StarIdentifiers` (HIP / HD / HR /
  proper name / catalogue designation + J2000 coordinates) and pure
  `simbad_query_url(&StarIdentifiers) -> String` /
  `vizier_query_url(&StarIdentifiers) -> String` builders. SIMBAD prefers
  an identifier query (`sim-id?Ident=…`, HIP → HD → HR → proper →
  designation) and falls back to a J2000 cone search (`sim-coo`); VizieR
  always uses a positional cone search (`VizieR-4?-c=…&-c.rs=…`). The
  builders are re-exported on the documented `stars_host_common`
  (`crates/common`) path; the pure helper lives in `catalog` so the WASM
  web binding shares one source of truth without taking the native-only
  `clap` / `chrono` dependencies.
- `crates/common/src/goto.rs`: `GotoTarget` gains `simbad_url` /
  `vizier_url` (`None` for solar-system bodies, which the CDS stellar
  archives do not catalogue), built from the identifiers the resolver
  already has (uses existing `L-17` / `L-18` paths; no new ingest).
- `apps/web` (`lib.rs`): `goto_object` JSON emits `simbadUrl` /
  `vizierUrl`; `apps/web/frontend` info panel renders the links behind an
  opt-in checkbox persisted in `localStorage` (default off).
- `apps/cli` / `apps/viewer`: echo the SIMBAD / VizieR URLs in the GoTo
  metadata output (`println!` / `log`). No network calls anywhere; the
  renderer is untouched.

**Tests / validation.**
- `catalog::links` (8 tests): identifier priority, deep-sky designation
  encoding, positive / negative declination sign handling, RA wraparound,
  and the coordinate fallback all match the CDS URL specification.
- `stars_host_common::goto` (3 tests): a named star resolves to a HIP
  `sim-id` link, a Messier object resolves to a designation `sim-id`
  link, and solar-system bodies expose no CDS links.
- Frontend: the web project ships no JS test harness, so the link
  rendering is covered by the Rust JSON-emission tests plus the `tsc`
  type-check in `make ci`; a `vitest` browser test is deferred with the
  rest of the frontend test-infra work.

**Hosts wired.** Web (info panel links). CLI / viewer expose the URLs in
the GoTo metadata output.

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

### `L-21` Python bindings (PyO3) — ⏳ astronomy queries + session round-trip shipped

**Shipped this rung.** A self-contained `bindings/python/` crate
(`stars-py`, `cdylib + rlib`) wrapping the `astronomy` + `catalog`
public surface through PyO3 0.22 with an `abi3-py39` ABI-stable build.
The wrapper exposes:

- `Observer` (lat / lon / UTC JD, plus a `from_unix_seconds` ctor for
  `datetime.timestamp()` notebook patterns),
- `apparent_sun_moon`, `apparent_planets`, `apparent_galilean_moons`,
  `apparent_titan` topocentric helpers,
- **observation-planning queries** — `evening_plan` (returning
  `EveningPlan` with `RiseTransitSet` rows + a `TwilightIndicator`
  timeline), `rise_transit_set(observer, body, start, end)`,
  `twilight_indicators`, `twilight_band`, and `body_altitude_rad`,
  wrapping `astronomy::planning` 1:1,
- **time helpers** `julian_date_from_unix_seconds` and
  `jd_utc_to_unix_ms` so notebooks can move between POSIX time and the
  JD planning windows,
- a mutable **`Session`** class that loads / saves the `crates/common`
  `StarSession` JSON schema (camelCase, current
  `SESSION_SCHEMA_VERSION`). It exposes typed observer / time / view
  accessors, recomputes the dependent time scales
  (`TimeScales::from_utc_julian_date`) when `jd_utc` is set, bridges to
  `Observer` via `.observer()` so queries reproduce the renderer's
  numerics from a loaded session, and preserves every other field
  (overlays, atmosphere, projection, eyepiece, corrections) byte-for-
  byte on round-trip. `Session` wraps the parsed JSON value rather than
  re-declaring the schema, and its constructors seed from the committed
  `dark-sky` preset (regenerated on every schema bump) so the binding
  never drifts from the real layout,
- `StarCatalog.load_embedded` over the V-23 compact binary baked at
  build time,
- `.altaz(observer)` on every apparent-body class so a notebook can
  read horizontal coordinates without re-implementing
  `equatorial_to_horizontal`.

The binding stays off the renderer / WGPU / CLI dependency path: the
session round-trip rides on `serde_json` alone, not the host
`stars-host-common` crate (which pulls in `clap` / `chrono` / `wgpu`).
The only side-effects are reading the embedded catalog and the optional
`Session.load` / `.save` file helpers. Wheel build is documented in
`bindings/python/README.md` (`maturin develop --features
extension-module`); a CI wheel matrix is tracked as the L-21 follow-up
scope.

**Gate.** `make pyo3-check` (`cargo check -p stars-py`) plus nine
in-crate Rust unit tests — Observer round-trip, planet-order match
with `apparent_planets_topocentric`, embedded-catalog load + index
error, a pure-Rust Moon-altitude smoke probe at the V-27 Tokyo epoch,
the evening-plan window contiguity + body-count contract, named-body
validation, the embedded-template schema guard, session
edit/round-trip with time-scale recomputation, and `from_observer`
time-scale preservation — are wired into `make ci`. The
Python-toolchain wheel build is opt-in via the `extension-module`
feature and **not** required by CI.

**Follow-up scope (still ⬜).**

- `pip install stars-py` wheel matrix built via maturin in CI for
  Linux / macOS / Windows. Needs a Python toolchain in the GitHub
  Actions job; the current rung documents the local `maturin develop`
  path only.
- Port `examples/notebooks/session_reproducibility.py` from CLI-render
  + JSON parsing onto the binding so the notebook directly cross-
  checks the renderer's numbers without a CLI shell-out. (The session
  round-trip + `evening_plan` surface this rung adds is the
  prerequisite; the notebook port itself is still open.)
- Expand to occultation / eclipse helpers (`active_occluders`,
  `find_lunar_occultation`, `find_solar_eclipse`) now that the
  planning + session base has stabilised.

**Tests / validation.**
- In-crate Rust unit tests exercising the wrapper types end-to-end
  through pure-Rust entry points (no interpreter needed in `make ci`).
- `bindings/python/tests/smoke.py` exercises the apparent-body,
  planning, and session round-trip surface after `maturin develop`.

**Hosts wired.** Bindings live alongside the existing hosts; not a host
itself.

---

### `L-22` Headless server mode — ✅

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
