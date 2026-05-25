# Roadmap

This document is the planning source of truth for `stars`: what has shipped,
what remains open, and what each phase is supposed to buy. The implementation
history is tracked separately in [`PROGRESS.md`](PROGRESS.md).

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

**Current highest priority:** continue the remaining visual Phase 4 work in a
small, shippable sequence: custom external viewpoint origins, deep-sky overlays,
and telescope eyepiece simulation. Phase 1, Phase 1', Phase 2, the core stellar
apparent-place corrections, full-sky projections, and the fixed external
galactic viewpoint are now complete. A row is `✅ done` only when the model
named in its references is implemented, documented, tested, and wired into all
relevant hosts.

### Atmosphere implementation ladder

To avoid conflating prototypes with roadmap completion, Phase 2 atmosphere work
is split into independently shippable rungs:

1. **Input plumbing** — Sun/Moon apparent directions, radii, phase, and host
   controls are available to the renderer. This is mostly present today.
2. **Renderable bodies** — Sun/Moon disks render from those inputs without
   polluting the star catalogue. This is present at a visual level, but still
   uses low-precision ephemerides.
3. **Daylight model** — daylight sky colour uses a cited sky model over its
   valid domain, without exposure cheats or star-visibility gates.
4. **Twilight model** — sun-below-horizon sky brightness is a real radiance
   model (or a clearly cited observational fit with pinned tests), continuous in
   time and direction, not a hand-tuned `smoothstep` fade.
5. **Validation** — noon, sunset, civil/nautical/astronomical twilight, and
   moonlit-night reference scenes are pinned by tests/screenshots and documented
   with the model limits.

There are currently no `⏳ next` atmosphere rows; the active Phase 2 queue has
moved on to solar-system bodies and planning UI.

---

## Work items

Columns:
- **ID**: stable roadmap item identifier (`P1-01`, `P1P-01`, etc.).
- **Phase**: which phase the row belongs to.
- **Item**: the deliverable. Bold = subsystem name; the rest is the
  one-line spec.
- **Reference / Notes**: literature citation for the standards-bearing
  items (Phases 1' / 2); short implementation note for the UX / platform
  items (Phases 1 / 3 / 4).
- **Status**: `✅ done` (path to landing in parens), `⏳ next`, `⬜` open.

| ID | Phase | Item | Reference / Notes | Status |
|---|---|---|---|---|
| `P1-01` | 1 | **Sky overlays library** | `crates/renderer/src/overlay.rs` | ✅ done |
| `P1-02` | 1 | **Overlay layers** — horizon, cardinals, alt-az grid, equatorial grid, ecliptic, celestial equator, meridian | All seven shipped in PR #1 | ✅ done |
| `P1-03` | 1 | **CLI flags** — `--overlays`, `--no-overlays`, `--grid-step-deg`, `--overlay-opacity` | `apps/cli` | ✅ done |
| `P1-04` | 1 | **Desktop viewer flags** — parity with CLI | `apps/viewer` | ✅ done |
| `P1-05` | 1 | **Web HUD redesign** — gear button + organized modal settings panel | `apps/web/frontend` | ✅ done |
| `P1-06` | 1 | **localStorage persistence** — observer + view survive reloads | `apps/web/frontend` | ✅ done |
| `P1-07` | 1 | **Web overlay toggles** — mirror CLI flags inside the settings panel | `apps/web/frontend` | ✅ done |
| `P1-08` | 1 | **Galactic equator overlay** | Same line pipeline as ecliptic; uses the transform from Phase 1' | ✅ done (`renderer::overlay`) |
| `P1-09` | 1 | **Constellation lines** — modern western stick figures | `crates/renderer/data/constellation_lines.csv`, derived from BSD-licensed d3-celestial line data, compacted by the renderer build script, and drawn by `renderer::overlay` | ✅ done (`renderer::constellations`, `renderer::overlay`) |
| `P1-10` | 1 | **Constellation boundaries** — IAU/Delporte regions | `crates/renderer/data/constellation_boundaries.csv`, derived from CDS VI/49 / Delporte 1930 B1875 boundary vertices precessed to J2000 and compacted by the renderer build script | ✅ done (`renderer::constellations`, `renderer::overlay`) |
| `P1-11` | 1 | **Star / planet / constellation labels** — star proper names + Bayer / Flamsteed for top ~50 stars, planet names, and constellation names | Built-in bitmap font atlas + screen-space label-placement pass; top-50 star labels and constellation label anchors are generated from HYG v4.2, solar-system labels use renderer apparent Sun/Moon/planet positions | ✅ done (`renderer::text`, `renderer::overlay`) |
| `P1-12` | 1 | **N / E / S / W and degree labels** | Same text atlas draws default cardinal labels and optional alt-az degree labels | ✅ done (`renderer::text`, `apps/{cli,viewer,web}`) |
| `P1P-01` | 1' | **Photometric zeropoint** — `magnitude → illuminance (lux)` so the whole pipeline runs in physical units | Schaefer, B. E. 1990, PASP 102, 212 | ✅ done (`astronomy::photometry`) |
| `P1P-02` | 1' | **Mesopic chromatic-fidelity weight** — log-linear blend over the 0.005–5 cd/m² mesopic range, applied per-star so only bright stars retain B-V colour | CIE 191:2010, *Recommended System for Mesopic Photometry Based on Visual Performance* | ✅ done (`astronomy::photometry`) |
| `P1P-03` | 1' | **Purkinje-shifted scotopic desaturation** — faint stars collapse toward a rod-weighted (~507 nm peak) grey rather than a flat luma | CIE 1951 V'(λ); Bowmaker & Dartnall 1980, J. Physiol. 298, 501 | ✅ done (`astronomy::photometry`) |
| `P1P-04` | 1' | **HDR render target** (`Rgba16Float`) — replace the 8-bit sRGB attachment so faint-star contributions accumulate instead of being crushed by the discard cutoff | Reinhard et al. 2002, *Photographic Tone Reproduction for Digital Images*, SIGGRAPH '02 | ✅ done (`renderer::tonemap`) |
| `P1P-05` | 1' | **Eye PSF / glare** — 3-component Spencer human PSF (sharp Gaussian core, 1/r³ lenticular halo, 1/r² corneal halo) + 4-point ciliary corona | Spencer, Shirley, Zimmerman & Greenberg 1995, SIGGRAPH '95; Ritschel et al. 2009, Eurographics | ✅ done (`shaders/star.wgsl`) |
| `P1P-06` | 1' | **Atmospheric extinction** — Kasten-Young 1989 airmass + per-channel Hardie 1962 / Schaefer 1993 coefficients, applied per-star in the vertex shader | Kasten & Young 1989, Applied Optics 28, 4735; Schaefer 1993, Vistas in Astronomy 36, 311; Hardie 1962, *Photoelectric Reductions* | ✅ done (`astronomy::photometry::airmass_kasten_young`, `renderer::Atmosphere`, `shaders/star.wgsl`) |
| `P1P-07` | 1' | **Diffuse sky background** — integrated-starlight + diffuse-galactic-light analytic fit to Leinert 1998 §6, evaluated per fragment in galactic coordinates as a fullscreen pass | Leinert et al. 1998, A&AS 127, 1; Roach & Megill 1961, ApJ 133, 228 | ✅ done (`astronomy::skyglow`, `renderer::skyglow`) |
| `P1P-08` | 1' | **Sky tone reproduction** — adaptive Reinhard 2002 §3.3 keyed operator with scene-luminance reduction + CIE 191:2010 mesopic regime split. Ferwerda 1996 TVI functions motivate the key selection; the tonemap pass now applies per-fragment rod/cone separation with a compact Pattanaik-style local adaptation luminance | Ferwerda et al. 1996, SIGGRAPH '96 (TVI Eqs. 1-2); Reinhard et al. 2002, SIGGRAPH '02 §3.2/3.3; CIE 191:2010 | ✅ done (`astronomy::photometry::{cone_tvi_log10, rod_tvi_log10, hdr_flux_to_luminance_cd_m2}`, `shaders/{luminance,tonemap}.wgsl`) |
| `P1P-09` | 1' | **Zodiacal light + airglow + interstellar dust** — Leinert-inspired sun-relative zodiacal-light band plus antisolar gegenschein, dark-site airglow floor, and analytic SFD-inspired dust extinction are summed in S10 flux units in both the Rust reference model and the skyglow shader | Leinert et al. 1998 §5 (ZL) and §7 (airglow); Schlegel, Finkbeiner & Davis 1998, ApJ 500, 525 (dust) | ✅ done (`astronomy::skyglow`, `shaders/skyglow.wgsl`) |
| `P1P-10` | 1' | **Per-fragment rod/cone tone reproduction** — tonemap computes fragment-local adaptation luminance, selects rod/cone response from the local CIE 191 mesopic state, and feeds the result through the Reinhard keyed operator | Pattanaik et al. 1998, SIGGRAPH '98; Ferwerda et al. 1996, SIGGRAPH '96; Durand & Dorsey 2002 (edge-aware local-adaptation refinement) | ✅ done (`shaders/tonemap.wgsl`) |
| `P1P-11` | 1' | **Catalogue colour pipeline upgrade** — B−V → T_eff → blackbody spectrum → CIE 1931 XYZ → sRGB, replacing the current piecewise-polynomial fit so the photopic input to the mesopic blend is physically calibrated | Ballesteros, F. J. 2012, EPL 97, 34008 | ✅ done (`catalog::color`) |
| `P2-01` | 2 | **Time systems** — separate UTC, UT1, TT, TAI; expose TDB for ephemerides | Built-in IERS/USNO leap-second table, explicit DUT1, TT, and approximate TDB exposed through `astronomy::TimeScales` and wired into all hosts | ✅ done (`astronomy::time`, `astronomy::Observer`) |
| `P2-02` | 2 | **Precession** — star positions of date instead of J2000 | IAU 2006 (P03) Fukushima-Williams precession matrix in `astronomy::corrections`, applied in the renderer camera uniform | ✅ done (`astronomy::corrections`, `renderer::camera`, `shaders/star.wgsl`) |
| `P2-03` | 2 | **Nutation** — ~9″ accuracy | Compact IAU-2000-style dominant luni-solar terms in `astronomy::corrections`, with equation-of-equinoxes sidereal-time wiring | ✅ done (`astronomy::corrections`, `renderer::camera`) |
| `P2-04` | 2 | **Annual aberration** — up to 20″ | First-order annual aberration from Earth orbital velocity, uploaded per frame and applied in the star shader | ✅ done (`astronomy::corrections`, `shaders/star.wgsl`) |
| `P2-05` | 2 | **Stellar proper motion** — apply HYG's `pmra` / `pmdec` when epoch ≠ catalog epoch | HYG `pmrarad` / `pmdecrad` are converted to Cartesian tangent vectors in both CSV and embedded catalogs and evaluated per frame | ✅ done (`catalog::coords`, `catalog::catalog`, `renderer::vertex`, `shaders/star.wgsl`) |
| `P2-06` | 2 | **Atmospheric refraction** — up to 34′ at the horizon | Saemundsson 1986-style apparent-altitude correction with pressure/temperature controls, applied to stars plus Sun/Moon disk directions | ✅ done (`astronomy::corrections`, `renderer::Atmosphere`, `renderer::camera`, `shaders/star.wgsl`, `apps/{cli,viewer,web}`) |
| `P2-07` | 2 | **Sun, Moon** — apparent topocentric direction, angular radius, phase, and disk rendering inputs | VSOP87/FK5 Sun + ELP2000-style Moon from `astro`, followed by WGS84 topocentric parallax; feeds scattering, twilight, moon phase, and disk rendering | ✅ done (`astronomy::ephemeris`, `renderer::skyglow`) |
| `P2-08` | 2 | **Solar / lunar illuminants** — spectral or XYZ irradiance for direct sunlight and moonlight at the top of the atmosphere | CIE daylight-basis / ASTM G-173-scale solar XYZ irradiance plus Krisciunas & Schaefer 1991 lunar phase photometry exposed as lux and XYZ | ✅ done (`astronomy::illuminants`) |
| `P2-09` | 2 | **Sunlit atmospheric scattering / sky colour** — Rayleigh + Mie aerosol + ozone absorption sky model driven by Sun altitude, view direction, observer altitude, and turbidity; produces blue daylight, golden-hour warmth, sunset reddening, and horizon haze | Preetham, Shirley & Smits 1999 daylight/Perez model, with renderer ozone and visibility controls plus Rust reference tests for daylight-domain luminance | ✅ done (`astronomy::atmosphere`, `shaders/skyglow.wgsl`) |
| `P2-10` | 2 | **Twilight and day/night blend** — combine sunlit scattering, moonlit sky, Phase 1' dark-sky glow, and star visibility using solar depression angle instead of hard-coded background colours | Solar-depression twilight radiance is continuous across civil / nautical / astronomical bands and composed additively with moonlit sky and Phase 1' dark-sky glow; Rust tests pin the model-domain boundaries | ✅ done (`astronomy::atmosphere`, `astronomy::skyglow`, `shaders/skyglow.wgsl`) |
| `P2-11` | 2 | **Atmosphere controls** — expose turbidity/aerosol, observer altitude, and optional ozone/visibility presets in CLI, viewer, and web settings | Defaults should match a clear rural sky; presets must be serializable in sessions | ✅ done (`apps/{cli,viewer,web}`, `crates/common`) |
| `P2-12` | 2 | **Planets (Mercury → Neptune)** — ~1″ on a century | VSOP87D light-time-corrected apparent planet states from `astro`, topocentric parallax, and planet disk/point rendering in the skyglow pass | ✅ done (`astronomy::ephemeris`, `renderer::camera`, `shaders/skyglow.wgsl`, `apps/{cli,viewer,web}`) |
| `P2-13` | 2 | **Moon phase + Earth-shadow** | Moon disk phase is renderer-driven and lunar-eclipse umbral contact is exposed as a darkening aid | ✅ done (`astronomy::ephemeris`, `shaders/skyglow.wgsl`) |
| `P2-14` | 2 | **Rise / transit / set tables** — per object, per evening | Local-evening Sun/Moon/planet rise-transit-set table in the web settings panel | ✅ done (`astronomy::planning`, `apps/web/frontend`) |
| `P2-15` | 2 | **Twilight indicators** — civil / nautical / astronomical bands | Solar-depression twilight labels plus planning-panel band intervals | ✅ done (`astronomy::planning`, `apps/web/frontend`) |
| `P2-16` | 2 | **Session URL** — encode (lat, lng, jd, az, alt, fov, overlays, planets, projection, atmosphere preset) | URL load/copy path using plain query parameters; no version gate | ✅ done (`apps/web/frontend`) |
| `P3-01` | 3 | **Hipparcos / Tycho-2 / Gaia DR3 ingest** | Pluggable catalog backend; keep HYG for the embedded WASM build | ⬜ |
| `P3-02` | 3 | **Identifier preservation** — Hipparcos / HD / TYC / Gaia source_id passed through the renderer | For hover / click-to-copy | ⬜ |
| `P3-03` | 3 | **SIMBAD / VizieR deep links** | Hover a star → external link with the right query | ⬜ |
| `P3-04` | 3 | **DE440 / VSOP87 ephemeris** | Move from "good enough for amateurs" (Phase 2) to publication-quality | ⬜ |
| `P3-05` | 3 | **Python bindings (PyO3)** | `astronomy` + `catalog` callable from Jupyter | ⬜ |
| `P3-06` | 3 | **Headless server mode** | HTTP service that returns PNGs (already 90% there in `apps/cli`) | ⬜ |
| `P3-07` | 3 | **Sharable JSON sessions** | Schema-versioned: observer + time + overlays + active corrections + catalog snapshot | ⬜ |
| `P3-08` | 3 | **`CITATION.cff` + Zenodo DOI** | Citable per-release artifact | ⬜ |
| `P3-09` | 3 | **Standards-compliance doc** | One page listing every IAU resolution / SOFA routine the code implements | ⬜ |
| `P4-01` | 4 | **Full-sky projections** | Mollweide, Aitoff, and Hammer all-sky maps selectable in CLI, desktop, and web; perspective remains the default | ✅ done (`renderer::SkyProjection`, `shaders/{star,skyglow,overlay}.wgsl`, `apps/{cli,viewer,web}`) |
| `P4-02` | 4 | **Out-of-Earth viewpoint** | `SkyViewpoint::GalacticNorth` moves the camera above the IAU galactic plane, places HYG stars by parsec distance, and draws an analytic top-down Milky Way disc in CLI, desktop, and web | ✅ done (`renderer::SkyViewpoint`, `shaders/{star,skyglow}.wgsl`, `apps/{cli,viewer,web}`) |
| `P4-03` | 4 | **Deep-sky overlay** (Messier, NGC) | Light catalogs first; full NGC/IC is large | ⬜ |
| `P4-04` | 4 | **Variable star light curves** | Pull AAVSO; show on the side panel for a hovered variable | ⬜ |
| `P4-05` | 4 | **Sound + screen-reader accessibility** | Az/Alt audio cues; ARIA labels on every control | ⬜ |
| `P4-06` | 4 | **Telescope eyepiece simulation** | Plate scale + true field of view from OTA + eyepiece pair | ⬜ |
| `P4-07` | 4 | **Custom external viewpoint origin** | Generalize `SkyViewpoint::GalacticNorth` into a user-selectable parsec-scale origin + orientation (`origin_pc`, target/up or yaw/pitch/roll), expose it in CLI / desktop / web session URLs, and document which coordinate frame is used | ⏳ next |

---

## Exit criteria

- **Phase 1.** Default config (web, native viewer, CLI) shows horizon +
  cardinal markers + an overlay system controllable from a single
  settings panel, with the seven sky-reference circles (horizon,
  cardinals, alt-az grid, equatorial grid, ecliptic, celestial equator,
  meridian), the galactic equator, constellation lines, constellation
  boundaries, star/planet/constellation labels, and cardinal/degree labels
  selectable per host.
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
- New native hosts (mobile, embedded) follow [`ARCHITECTURE.md`](ARCHITECTURE.md), no
  exceptions; if the recipe stops fitting, update `ARCHITECTURE.md` in the same
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
