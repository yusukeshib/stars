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

## Phase intent

| Phase | Theme | What it buys |
|---|---|---|
| **1** | Educational planetarium | UX — identify what you're looking at |
| **1'** | Physically-based visual pipeline | The sky *looks* like the sky a dark-adapted human would see |
| **2** | Observation planning tool | Positional precision — IAU-grade time + corrections |
| **3** | Research / education platform | Citability, JPL Horizons interop, notebook bindings |
| **4** | Dessert | Niche but high-value polish |

**Phase 1'** (read "Phase 1 prime") is named with a prime mark rather
than a decimal because it is an **orthogonal companion** to Phase 1, not
a sequential successor. It is a continuation of Phase 1's user-
experience axis ("the sky looks right") but is independent of every
other phase: Phase 2 (positional precision) does not depend on it,
Phase 1 does not depend on it, and it does not contribute to or unlock
either. The two halves of "the sky looks like the sky" — identifying
what's there (Phase 1) and rendering it physically correctly (Phase 1')
— can land in either order.

The work originally landed in PRs #7–#11 under the label "Phase 2.5",
which incorrectly implied it slotted between Phases 2 and 3. The git
history retains the legacy name; everything else uses "Phase 1'".

**Exit criteria** for each phase live at the end of this document, alongside
the rationale ordering. The status column below is the source of truth for
what's done and what's left.

### Atmosphere-rendering scope

Phase 1' covers the **dark-sky** atmosphere: stellar extinction, night-sky
background, zodiacal light, airglow, dust, glare, and human eye adaptation. It
intentionally does **not** model the blue daytime sky, sunset reddening, or the
colour of twilight caused by sunlight scattering through air.

That missing piece is tracked in Phase 2, after the Sun and Moon have apparent
topocentric positions. The goal is for sky colour to be driven by physical
illuminants and atmosphere parameters rather than by hard-coded gradients.

**Current highest priority:** implement the Sun/Moon inputs and sunlit
atmospheric-scattering path before the remaining Phase 2 precision work. In the
status column, these rows are marked `⏳ next`.

---

## Work items

Columns:
- **Phase**: which phase the row belongs to.
- **Item**: the deliverable. Bold = subsystem name; the rest is the
  one-line spec.
- **Reference / Notes**: literature citation for the standards-bearing
  items (Phases 1' / 2); short implementation note for the UX / platform
  items (Phases 1 / 3 / 4).
- **Status**: `✅ done` (path to landing in parens), `⏳ next`, `⬜` open.

| Phase | Item | Reference / Notes | Status |
|---|---|---|---|
| 1 | **Sky overlays library** | `crates/renderer/src/overlay.rs` | ✅ done |
| 1 | **Overlay layers** — horizon, cardinals, alt-az grid, equatorial grid, ecliptic, celestial equator, meridian | All seven shipped in PR #1 | ✅ done |
| 1 | **CLI flags** — `--overlays`, `--no-overlays`, `--grid-step-deg`, `--overlay-opacity` | `apps/cli` | ✅ done |
| 1 | **Desktop viewer flags** — parity with CLI | `apps/viewer` | ✅ done |
| 1 | **Web HUD redesign** — gear button + modal settings panel | `apps/web/frontend` | ✅ done |
| 1 | **localStorage persistence** — observer + view survive reloads | `apps/web/frontend` | ✅ done |
| 1 | **Web overlay toggles** — mirror CLI flags inside the settings panel | `apps/web/frontend` | ✅ done |
| 1 | **Galactic equator overlay** | Same line pipeline as ecliptic; uses the transform from Phase 1' | ✅ done (`renderer::overlay`) |
| 1' | **Photometric zeropoint** — `magnitude → illuminance (lux)` so the whole pipeline runs in physical units | Schaefer, B. E. 1990, PASP 102, 212 | ✅ done (`astronomy::photometry`) |
| 1' | **Mesopic chromatic-fidelity weight** — log-linear blend over the 0.005–5 cd/m² mesopic range, applied per-star so only bright stars retain B-V colour | CIE 191:2010, *Recommended System for Mesopic Photometry Based on Visual Performance* | ✅ done (`astronomy::photometry`) |
| 1' | **Purkinje-shifted scotopic desaturation** — faint stars collapse toward a rod-weighted (~507 nm peak) grey rather than a flat luma | CIE 1951 V'(λ); Bowmaker & Dartnall 1980, J. Physiol. 298, 501 | ✅ done (`astronomy::photometry`) |
| 1' | **HDR render target** (`Rgba16Float`) — replace the 8-bit sRGB attachment so faint-star contributions accumulate instead of being crushed by the discard cutoff | Reinhard et al. 2002, *Photographic Tone Reproduction for Digital Images*, SIGGRAPH '02 | ✅ done (`renderer::tonemap`) |
| 1' | **Eye PSF / glare** — 3-component Spencer human PSF (sharp Gaussian core, 1/r³ lenticular halo, 1/r² corneal halo) + 4-point ciliary corona | Spencer, Shirley, Zimmerman & Greenberg 1995, SIGGRAPH '95; Ritschel et al. 2009, Eurographics | ✅ done (`shaders/star.wgsl`) |
| 1' | **Atmospheric extinction** — Kasten-Young 1989 airmass + per-channel Hardie 1962 / Schaefer 1993 coefficients, applied per-star in the vertex shader | Kasten & Young 1989, Applied Optics 28, 4735; Schaefer 1993, Vistas in Astronomy 36, 311; Hardie 1962, *Photoelectric Reductions* | ✅ done (`astronomy::photometry::airmass_kasten_young`, `renderer::Atmosphere`, `shaders/star.wgsl`) |
| 1' | **Diffuse sky background** — integrated-starlight + diffuse-galactic-light analytic fit to Leinert 1998 §6, evaluated per fragment in galactic coordinates as a fullscreen pass | Leinert et al. 1998, A&AS 127, 1; Roach & Megill 1961, ApJ 133, 228 | ✅ done (`astronomy::skyglow`, `renderer::skyglow`) |
| 1' | **Sky tone reproduction** — adaptive Reinhard 2002 §3.3 keyed operator with scene-luminance reduction + CIE 191:2010 mesopic regime split. Ferwerda 1996 TVI functions motivate the key selection but per-fragment rod/cone separation is deferred | Ferwerda et al. 1996, SIGGRAPH '96 (TVI Eqs. 1-2); Reinhard et al. 2002, SIGGRAPH '02 §3.2/3.3; CIE 191:2010 | ✅ done (`astronomy::photometry::{cone_tvi_log10, rod_tvi_log10, hdr_flux_to_luminance_cd_m2}`, `shaders/{luminance,tonemap}.wgsl`) |
| 1' | **Zodiacal light + airglow + interstellar dust** — extend the skyglow pass with the remaining diffuse-sky components: Leinert ZL table (ecliptic coords, sun-relative longitude), broadly-isotropic airglow, Schlegel-Finkbeiner-Davis dust extinction map modulating ISL | Leinert et al. 1998 §5 (ZL) and §7 (airglow); Schlegel, Finkbeiner & Davis 1998, ApJ 500, 525 (dust) | ✅ done (`astronomy::skyglow`, `shaders/skyglow.wgsl`) |
| 1' | **Per-fragment rod/cone tone reproduction** — Pattanaik 1998 multiscale model with V'(λ)-weighted scotopic chroma; should derive the empirical `KEY_SCOTOPIC` in `tonemap.wgsl` analytically | Pattanaik et al. 1998, SIGGRAPH '98; Durand & Dorsey 2002 (bilateral local-adaptation refinement) | ✅ done (`shaders/tonemap.wgsl`) |
| 1' | **Catalogue colour pipeline upgrade** — B−V → T_eff → blackbody spectrum → CIE 1931 XYZ → sRGB, replacing the current piecewise-polynomial fit so the photopic input to the mesopic blend is physically calibrated | Ballesteros, F. J. 2012, EPL 97, 34008 | ✅ done (`catalog::color`) |
| 2 | **Time systems** — separate UTC, UT1, TT, TAI; expose TDB for ephemerides | Underpins everything below; pulls in a leap-second table | ⬜ |
| 2 | **Precession** — star positions of date instead of J2000 | IAU 2006 (P03) | ⬜ |
| 2 | **Nutation** — ~9″ accuracy | IAU 2000A or 2000B | ⬜ |
| 2 | **Annual aberration** — up to 20″ | Standard formulas; folds into the equatorial→ENU matrix | ⬜ |
| 2 | **Stellar proper motion** — apply HYG's `pmra` / `pmdec` when epoch ≠ catalog epoch | HYG carries the columns already | ⬜ |
| 2 | **Atmospheric refraction** — up to 34′ at the horizon | Bennett 1982 / Saemundsson 1986; flag in UI when on | ⬜ |
| 2 | **Sun, Moon** — apparent topocentric direction, angular radius, phase, and disk rendering inputs | VSOP87 (Sun) + ELP2000 (Moon); feeds scattering, twilight, moon phase, and rise/set | ✅ done (`astronomy::ephemeris::SunMoonApparent`, `renderer::CameraUniform`) |
| 2 | **Solar / lunar illuminants** — spectral or XYZ irradiance for direct sunlight and moonlight at the top of the atmosphere | ASTM G-173 / CIE daylight basis for Sun; Krisciunas & Schaefer 1991 for moonlight brightness | ✅ done (`astronomy::illuminants`) |
| 2 | **Sunlit atmospheric scattering / sky colour** — Rayleigh + Mie aerosol + ozone absorption sky model driven by Sun altitude, view direction, observer altitude, and turbidity; produces blue daylight, golden-hour warmth, sunset reddening, and horizon haze | Preetham, Shirley & Smits 1999; Hosek & Wilkie 2012; Bruneton & Neyret 2008 | ✅ done (`shaders/skyglow.wgsl`, `renderer::Atmosphere`) |
| 2 | **Twilight and day/night blend** — combine sunlit scattering, moonlit sky, Phase 1' dark-sky glow, and star visibility using solar depression angle instead of hard-coded background colours | Civil / nautical / astronomical bands remain UI annotations; renderer cross-fades radiance physically across 0°, −6°, −12°, −18° Sun altitude. Basic shader blending exists; UI annotations and physically validated cross-fade remain. | ⏳ next |
| 2 | **Atmosphere controls** — expose turbidity/aerosol, observer altitude, and optional ozone/visibility presets in CLI, viewer, and web settings | Defaults should match a clear rural sky; presets must be serializable in sessions. CLI/viewer/web controls exist for preset/turbidity/altitude; ozone/visibility presets and session URL serialization remain. | ⏳ next |
| 2 | **Planets (Mercury → Neptune)** — ~1″ on a century | VSOP87 truncated | ⬜ |
| 2 | **Moon phase + Earth-shadow** | Visual aid; trivial once Moon ephemeris lands | ⬜ |
| 2 | **Rise / transit / set tables** — per object, per evening | UI table in the settings panel | ⬜ |
| 2 | **Twilight indicators** — civil / nautical / astronomical bands | Time slider annotation plus labels for the scattering blend state | ⬜ |
| 2 | **Session URL** — encode (lat, lng, jd, az, alt, fov, overlays, planets, atmosphere preset) | One URL, schema-versioned | ⬜ |
| 3 | **Hipparcos / Tycho-2 / Gaia DR3 ingest** | Pluggable catalog backend; keep HYG for the embedded WASM build | ⬜ |
| 3 | **Identifier preservation** — Hipparcos / HD / TYC / Gaia source_id passed through the renderer | For hover / click-to-copy | ⬜ |
| 3 | **SIMBAD / VizieR deep links** | Hover a star → external link with the right query | ⬜ |
| 3 | **DE440 / VSOP87 ephemeris** | Move from "good enough for amateurs" (Phase 2) to publication-quality | ⬜ |
| 3 | **Python bindings (PyO3)** | `astronomy` + `catalog` callable from Jupyter | ⬜ |
| 3 | **Headless server mode** | HTTP service that returns PNGs (already 90% there in `apps/cli`) | ⬜ |
| 3 | **Sharable JSON sessions** | Schema-versioned: observer + time + overlays + active corrections + catalog snapshot | ⬜ |
| 3 | **`CITATION.cff` + Zenodo DOI** | Citable per-release artifact | ⬜ |
| 3 | **Standards-compliance doc** | One page listing every IAU resolution / SOFA routine the code implements | ⬜ |
| 4 | **Full-sky projections** | Mollweide, Aitoff, Hammer; required to show galactic / extragalactic structure | ⬜ |
| 4 | **Out-of-Earth viewpoint** | Camera not centered on Earth; render the Milky Way disc from above | ⬜ |
| 4 | **Deep-sky overlay** (Messier, NGC) | Light catalogs first; full NGC/IC is large | ⬜ |
| 4 | **Variable star light curves** | Pull AAVSO; show on the side panel for a hovered variable | ⬜ |
| 4 | **Sound + screen-reader accessibility** | Az/Alt audio cues; ARIA labels on every control | ⬜ |
| 4 | **Telescope eyepiece simulation** | Plate scale + true field of view from OTA + eyepiece pair | ⬜ |

---

## Exit criteria

- **Phase 1.** Default config (web, native viewer, CLI) shows horizon +
  cardinal markers + an overlay system controllable from a single
  settings panel, with the seven sky-reference circles (horizon,
  cardinals, alt-az grid, equatorial grid, ecliptic, celestial equator,
  meridian) plus the galactic equator selectable per host.
- **Phase 1'.** Default-on rendering with a dark observer shows a visible
  Milky Way band, atmospheric reddening near the horizon, and a clear
  chromatic / achromatic split between bright and faint stars, with every
  numerical choice traceable to one of the references above via a doc
  comment.
- **Phase 2.** A documented switch table on `Observer` says which
  corrections are active; default-on subset matches what Stellarium calls
  "general" precision; differences against JPL Horizons for a fixed set
  of targets are < 1″ in unit tests. A new `astronomy::corrections`
  module collects the IAU-2006-compliant transforms. Daylight and twilight
  rendering are no longer hard-coded colours: Sun/Moon illuminants plus
  atmosphere parameters determine the sky radiance, with documented presets
  and screenshots covering noon, sunset, civil/nautical/astronomical twilight,
  and moonlit night.
- **Phase 3.** Someone can `pip install` or `cargo add` the relevant
  pieces, render the same sky from a notebook and the web UI, get the
  same numbers as JPL Horizons within stated tolerances, and cite the
  project in a paper.

---

## How to contribute against this plan

- Phase milestones live as GitHub issues tagged `phase-1` / `phase-1-prime` /
  `phase-2` / etc.
- Each row in the table above maps to one or two issues; pick one and
  open a PR.
- New native hosts (mobile, embedded) follow [`USAGE.md`](USAGE.md), no
  exceptions; if the recipe stops fitting, update `USAGE.md` in the same
  PR.
- Any change to `astronomy` that affects numerical output must come with
  a unit test that pins the value (we're aiming for "trustworthy" —
  silent numerical drift is the failure mode to avoid).

---

## Why these phases in this order

Phase 1 buys **user experience**. Without overlays nobody can tell what
they're looking at, and a few lines per frame is the cheapest thing in
the pipeline.

Phase 1' buys **visual realism**. Without the diffuse-sky pass, the
Milky Way is invisible; without atmospheric extinction, horizon stars
look exactly like zenith stars; without HDR + Spencer PSF, bright stars
look identical to faint ones modulo a single discard cutoff. These are
the things that make the difference between "a star plot" and "the sky".

Phase 2 buys **trust**. Without precession alone, star positions drift
~50″/year — within a decade the labels are visibly wrong. Refraction and
proper motion close the loop for naked-eye observers. The Sun/Moon work also
unlocks physically driven sky colour: the renderer can know where the primary
illuminants are, then let Rayleigh/Mie scattering and aerosol/turbidity controls
explain blue daylight, red sunsets, twilight gradients, and moonlit nights.

Phase 3 buys **reach**. The Rust + wgpu + WASM combo applied to IAU-grade
astronomy is genuinely under-served: it lets the same engine power a
notebook plot, a web app, a CLI render, and a citation in a paper. That
is the part of this roadmap that turns the project from "another star
app" into "a thing other people build on."

Phase 4 is dessert.
