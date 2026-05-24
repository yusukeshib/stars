# Roadmap

The aim is for `stars` to be useful both as a casual night-sky viewer and as a
piece of software that astronomers, educators, and researchers can defend
using. Two axes drive the work:

1. **Pedagogy** — let users *see* the relationship between coordinate systems
   (equatorial, horizontal, ecliptic, galactic), reference circles, and time.
2. **Precision** — match the IAU's published standards for the conversions we
   perform, so positions on screen are defensible to arcsecond level and the
   choice of model is explicit.

The phases below are written so each one is independently shippable: anyone
can stop at the end of a phase and have a coherent product. Estimates assume
weekend-evening pace by one contributor.

---

## Phase 1 — "Educational planetarium" *(in progress)*

> Goal: a stargazer can identify what they're looking at and which way is
> which, on web, desktop, and from a one-shot CLI render.

| Item | Status |
|---|---|
| Sky overlays library (`crates/renderer/src/overlay.rs`) | ✅ done |
| Layers: horizon, cardinals, alt-az grid, equatorial grid, ecliptic, celestial equator, meridian | ✅ done |
| CLI flags (`--overlays`, `--no-overlays`, `--grid-step-deg`, `--overlay-opacity`) | ✅ done |
| Desktop viewer flags (parity with CLI) | ✅ done |
| Web HUD redesign (gear button + modal settings panel) | ✅ done |
| localStorage persistence of observer + view | ✅ done |
| Web overlay toggles (mirror CLI flags inside the settings panel) | ⏳ next |
| Constellation lines (IAU asterisms) | ⬜ |
| Constellation boundaries (IAU 1930) | ⬜ |
| Star proper names + Bayer / Flamsteed labels for top ~50 stars | ⬜ |
| Galactic equator overlay (l, b) | ⬜ |
| Anti-aliased / thickness-controllable lines (triangle-strip line rendering) | ⬜ |
| N / E / S / W and degree labels rendered as text (font atlas) | ⬜ |

**Exit criteria for Phase 1.** Default config (web, native viewer, CLI) shows
horizon + cardinal markers + constellation lines, with full overlay control
behind a single settings panel.

---

## Phase 2 — "Observation planning tool"

> Goal: an amateur astronomer can trust the times and positions enough to plan
> a session: when does Vega rise, where will the moon be at 22:00, what's the
> sun-altitude through twilight.

| Item | Notes |
|---|---|
| **Time systems** — separate UTC, UT1, TT, TAI; expose TDB for ephemerides | Underpins everything below; pulls in a leap-second table |
| **Precession** — IAU 2006 (P03) | Star positions of date instead of J2000 |
| **Nutation** — IAU 2000A or 2000B | ~9″ accuracy |
| **Annual aberration** | Up to 20″ |
| **Stellar proper motion** | HYG already carries `pmra` / `pmdec`; apply them when epoch ≠ catalog epoch |
| **Atmospheric refraction** | Up to 34′ at the horizon; flag it in UI when on |
| **Sun, Moon** | VSOP87 or ELP2000 |
| **Planets (Mercury → Neptune)** | VSOP87 truncated, ~1″ on a century |
| **Moon phase + Earth-shadow** | Visual aid; trivial once Moon ephemeris lands |
| **Rise / transit / set tables** | Per object, per evening |
| **Twilight indicators** | Civil / nautical / astronomical bands on the time slider |
| **Session URL** | Encode (lat, lng, jd, az, alt, fov, overlays, planets) as one URL |

A new `astronomy::corrections` module collects the IAU-2006-compliant
transforms, kept Earth-rotation-only on the bottom (no relativistic / TDB-TT
chain unless explicitly needed).

**Exit criteria for Phase 2.** A documented switch table on `Observer` says
which corrections are active; default-on subset matches what Stellarium calls
"general" precision; differences against JPL Horizons for a fixed set of
targets are < 1″ in unit tests.

---

## Phase 2.5 — "Physically-based visual pipeline"

> Goal: what's on screen is what a dark-adapted human would actually see
> from the configured observer + time + atmosphere, with every step of the
> visual chain anchored to a published standard or peer-reviewed paper —
> no artistic tuning, no decorative textures. The Milky Way appears because
> the diffuse galactic light is *included as a physical quantity*, not
> painted in. Bright stars look coloured and faint stars look grey because
> the rod/cone model says so. Stars near the horizon redden because the
> atmosphere is in the chain.

This phase sits between Phase 1 (overlays — "what am I looking at") and
Phase 2 (positional precision — "is it in the right place"), because
perceptual realism is a prerequisite for the project to *visually* match
the night sky users know.

| Item | Reference | Status |
|---|---|---|
| **Photometric zeropoint** — `magnitude → illuminance (lux)` so the whole pipeline runs in physical units | Schaefer, B. E. 1990, PASP 102, 212 | ✅ done (`astronomy::photometry`) |
| **Mesopic chromatic-fidelity weight** — log-linear blend over the 0.005–5 cd/m² mesopic range, applied per-star so only bright stars retain B-V colour | CIE 191:2010 *Recommended System for Mesopic Photometry Based on Visual Performance* | ✅ done (`astronomy::photometry`) |
| **Purkinje-shifted scotopic desaturation** — faint stars collapse toward a rod-weighted (~507 nm peak) grey rather than a flat luma | CIE 1951 V'(λ); Bowmaker & Dartnall 1980, J. Physiol. 298, 501 | ✅ done (`astronomy::photometry`) |
| **HDR render target** (`Rgba16Float`) — replace the 8-bit sRGB attachment so faint-star contributions accumulate instead of being crushed by the discard cutoff | Reinhard et al. 2002, *Photographic Tone Reproduction for Digital Images*, SIGGRAPH '02 | ✅ done (`renderer::tonemap`) |
| **Eye PSF / glare** — replace the bare Gaussian sprite with the 3-component Spencer human PSF (sharp Gaussian core, 1/r³ lenticular halo, 1/r² corneal halo) plus a 4-point ciliary corona, so bright stars get physically correct extent and faint stars combine via the wings | Spencer, Shirley, Zimmerman & Greenberg 1995, *Physically-based glare effects for digital images*, SIGGRAPH '95; Ritschel et al. 2009, *Temporal Glare*, Eurographics | ✅ done (`shaders/star.wgsl`) |
| **Atmospheric extinction** — Kasten-Young 1989 airmass + per-channel Schaefer 1993 / Hardie 1962 coefficients, applied per-star in the vertex shader; below-horizon stars are culled when extinction is enabled | Kasten & Young 1989, *Revised optical air mass tables*, Applied Optics 28, 4735; Schaefer 1993, *Astronomy and the limits of vision*, Vistas in Astronomy 36, 311; Hardie 1962, *Photoelectric Reductions* | ✅ done (`astronomy::photometry::airmass_kasten_young`, `renderer::Atmosphere`, `shaders/star.wgsl`) |
| **Diffuse sky background** — integrated-starlight + diffuse-galactic-light analytic fit to Leinert 1998 §6, evaluated per fragment in galactic coordinates as a fullscreen pass before the star pass; atmospheric extinction applies. Zodiacal light + airglow + the Schlegel et al. dust map remain for a follow-up upgrade | Leinert et al. 1998, *The 1997 reference of diffuse night sky brightness*, A&AS 127, 1; Roach & Megill 1961, *Integrated starlight over the sky*, ApJ 133, 228 | ✅ done (`astronomy::skyglow`, `renderer::skyglow`) |
| **Sky tone reproduction** — adaptive Reinhard 2002 §3.3 photographic operator with scene-luminance reduction pass and CIE 191:2010 mesopic-aware key selection (low-key for night-adapted scenes, Zone-V for photopic). Ferwerda 1996 TVI functions are implemented in `astronomy::photometry` and motivate the key-selection regime split, but the actual per-channel rod/cone separation (V'(λ)-weighted scotopic chroma in the fragment) is deferred to a Pattanaik 1998 multiscale follow-up | Ferwerda, Pattanaik, Shirley & Greenberg 1996, *A Model of Visual Adaptation for Realistic Image Synthesis*, SIGGRAPH '96 (TVI Eqs. 1–2); Reinhard et al. 2002 SIGGRAPH '02 §3.2/3.3 (keyed operator); CIE 191:2010 (mesopic regime) | ✅ done (`astronomy::photometry::{cone_tvi_log10, rod_tvi_log10, hdr_flux_to_luminance_cd_m2}`, `shaders/luminance.wgsl`, `shaders/tonemap.wgsl`) |
| **Per-fragment rod/cone tone reproduction** — Pattanaik 1998 multiscale model: rods and cones processed as separate channels, scotopic regions automatically desaturated through V'(λ)-weighted luma (replacing the current Reinhard-key-only mesopic split, which approximates regime *intensity* but not per-pixel *chroma*). Should subsume the empirical `KEY_SCOTOPIC` constant in `tonemap.wgsl` by deriving the key analytically from the local adaptation state. | Pattanaik, S. N., Ferwerda, J. A., Fairchild, M. D., & Greenberg, D. P. 1998, *A Multiscale Model of Adaptation and Spatial Vision for Realistic Image Display*, SIGGRAPH '98; Durand & Dorsey 2002 (bilateral local-adaptation refinement) | ⬜ |
| **Catalogue colour pipeline upgrade** — B−V → T_eff (Ballesteros 2012) → blackbody spectrum → CIE 1931 XYZ → sRGB, so the photopic input to the mesopic blend is itself physically calibrated | Ballesteros, F. J. 2012, EPL 97, 34008 | ⬜ |

**Exit criteria for Phase 2.5.** Default-on rendering with a dark observer
shows a visible Milky Way band, atmospheric reddening near the horizon, and
a clear chromatic / achromatic split between bright and faint stars, with
every numerical choice traceable to one of the references above via a doc
comment.

Why this slot in the roadmap: the goal of the project is that what shows on
screen is *defensible* — both in position (Phase 2) and in appearance
(Phase 2.5). Going to Phase 3 without 2.5 means publishing an engine that's
numerically careful about positions but visually misleading about what a
rural sky looks like. The two pieces are independent and can ship in either
order.

---

## Phase 3 — "Research / education platform"

> Goal: stars can sit in a notebook, a paper, or a syllabus. Reproducibility
> and citability are first-class.

| Item | Notes |
|---|---|
| **Hipparcos / Tycho-2 / Gaia DR3 ingest** | Replace HYG-only path with a pluggable catalog backend; keep HYG for the embedded WASM build |
| **Identifier preservation** | Hipparcos / HD / TYC / Gaia source_id passed through the renderer for hover / click-to-copy |
| **SIMBAD / VizieR deep links** | Hover a star → external link with the right query |
| **DE440 / VSOP87 ephemeris** | Move from "good enough for amateurs" (Phase 2) to publication-quality |
| **Python bindings (PyO3)** | `astronomy` and `catalog` callable from Jupyter; one-line "what was the sky over Tokyo at 2024-04-08T15:00Z?" recipe |
| **Headless server mode** | HTTP service that returns PNGs (already 90% there in `apps/cli`; needs a thin server wrapper) |
| **Sharable JSON sessions** | Schema-versioned config: observer + time + overlays + active corrections + catalog snapshot |
| **`CITATION.cff` + Zenodo DOI** | Citable per-release artifact |
| **Standards-compliance doc** | One markdown page listing every IAU resolution / SOFA routine the code implements, with the version |

**Exit criteria for Phase 3.** Someone can `pip install` or `cargo add` the
relevant pieces, render the same sky from a notebook and the web UI, get the
same numbers as JPL Horizons within stated tolerances, and cite the project
in a paper.

---

## Phase 4 — Niche, but high-value once everything above lands

| Item | Notes |
|---|---|
| **Full-sky projections** (Mollweide, Aitoff, Hammer) | Toggle from camera; required to show galactic / extragalactic structure |
| **Out-of-Earth viewpoint** | Camera not centered on Earth; render the Milky Way disc from above, visualize 3D parallax |
| **Deep-sky overlay** (Messier, NGC) | Light catalogs first; full NGC/IC is large |
| **Variable star light curves** | Pull AAVSO; show on the side panel for a hovered variable |
| **Sound + screen-reader accessibility** | Az/Alt audio cues; ARIA labels on every control |
| **Telescope eyepiece simulation** | Plate scale, true field of view, given an OTA + eyepiece pair |

---

## How to contribute against this plan

- Phase milestones live as GitHub issues tagged `phase-1` / `phase-2` / etc.
- Each row in the tables above maps to one or two issues; pick one and open a PR.
- New native hosts (mobile, embedded) follow [`USAGE.md`](USAGE.md), no
  exceptions; if the recipe stops fitting, update `USAGE.md` in the same PR.
- Any change to `astronomy` that affects numerical output must come with a
  unit test that pins the value (we're aiming for "trustworthy" — silent
  numerical drift is the failure mode to avoid).

---

## Why these phases in this order

Phase 1 buys **user experience**. Without overlays nobody can tell what
they're looking at, and a few lines per frame is the cheapest thing in the
pipeline.

Phase 2 buys **trust**. Without precession alone, star positions drift
~50″/year — within a decade the labels are visibly wrong. Refraction and
proper motion close the loop for naked-eye observers.

Phase 3 buys **reach**. The Rust + wgpu + WASM combo applied to IAU-grade
astronomy is genuinely under-served: it lets the same engine power a
notebook plot, a web app, a CLI render, and a citation in a paper. That is
the part of this roadmap that turns the project from "another star app" into
"a thing other people build on."

Phase 4 is dessert.
