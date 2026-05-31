# Data sources

This document records external data and literature-derived inputs used by
`stars`. Keep it updated whenever adding catalog files, generated datasets,
shader lookup data, scene presets, validation-gallery inputs, telescope presets,
or coefficients copied from a source.

## Machine-readable provenance manifest

The deterministic manifest lives at [`data/manifest.toml`](data/manifest.toml).
It is the source-of-truth shape that CI verifies, while this document is the
human-readable narrative. Every committed data artifact, every generated /
regenerable artifact, and every runtime web service the application calls has
an entry there with:

- exact source URL, archive identifier, or literature citation;
- version / release date and retrieval date;
- license and redistribution terms;
- local path and SHA-256 hash;
- preprocessing command or generator version;
- fields used by `stars`;
- known limitations and whether the artifact is `embedded`, `generated`, or
  `runtime-service`.

The manifest schema is defined and validated by the [`stars-manifest`](crates/manifest)
crate. `make manifest-check` (wired into `make ci`) recomputes SHA-256 over
every local artifact and fails if any byte has drifted from the recorded
hash, so adding or changing a data file in a PR without updating its entry
is a CI failure.

Downstream consumers cite artifacts by stable `id`. JSON sessions name the
catalog snapshot they used via the manifest id (see
[`ARCHITECTURE.md`](ARCHITECTURE.md) and `docs/catalog-backend-design.md`); a
future Gaia / Tycho / Hipparcos backend (roadmap `L-17`) will add new
artifacts under that contract.

When adding a new data file, append a row to `data/manifest.toml` in the
same PR. The required fields per `kind` are:

- `embedded` — `path`, `sha256`, `license`, `source` (or `citation`).
- `generated` — same as `embedded` plus `preprocessing` (regeneration command).
- `runtime-service` — `endpoint`, `license`; no `path` and no `sha256`.

## Star catalog

### HYG Database v4.2

Repository location:

- `crates/catalog/data/hyg_v42.csv`

Manifest id:

- `hyg-v4.2` in `data/manifest.toml`.

Used for:

- embedded and filesystem star catalog loading through the HYG backend scaffold;
- star positions;
- magnitudes;
- B−V colour values;
- proper motion where available;
- distance filtering;
- HYG `id`, `hip`, and `hd` numeric identifier preservation in
  `catalog::CatalogIdentifiers` for future hover/copy/session work;
- generated top-50 bright-star labels and bright-star-weighted constellation
  label anchors in `crates/renderer/build.rs`;
- bright named-star search index emitted as
  `crates/catalog/data/named_stars.tsv` (~1.2k entries with proper /
  Bayer / Flamsteed / HR / HD / HIP identifiers; manifest id
  `named-stars-search-index`) for the V-56 object-search rung. The compact
  rendered catalog is anonymous on the embedded path, so this narrower
  TSV is the only place where catalog names survive into the WASM build.

Acquisition:

```bash
./scripts/download-catalog.sh
```

Implementation areas:

- `crates/catalog/src/backend.rs`
- `crates/catalog/src/catalog.rs`
- `crates/catalog/src/coords.rs`
- `crates/catalog/src/color.rs`
- `crates/catalog/build.rs`

Current filtering policy:

- rows fainter than magnitude 8 are dropped;
- rows with HYG's `100000` parsec sentinel for unknown parallax are dropped.

Notes:

- `docs/catalog-backend-design.md` defines the backend, identifier, LOD, paging,
  and WASM-subset policy for future large-catalog ingest.

### Large-catalog ingest: Hipparcos / Tycho-2 / Gaia DR3 (`L-17`)

`crates/catalog/src/ingest.rs` implements three additional backends behind the
`CatalogBackend` trait. They parse the **normalised CSV export** of each
catalogue by column name (not the fragile fixed-width native records); the
companion fetch scripts request exactly those columns.

| Catalogue | VizieR / archive | Astrometry | Backend | Fetch script | Manifest id |
|---|---|---|---|---|---|
| Hipparcos main | I/239/hip_main | ≈1 mas | `HipparcosCsvBackend` | `scripts/fetch-hipparcos.sh` | `hipparcos-i239-main` |
| Tycho-2 | I/259/tyc2 | ≈60 mas | `Tycho2CsvBackend` | `scripts/fetch-tycho2.sh` | `tycho2-i259` |
| Gaia DR3 | `gaiadr3.gaia_source` / I/355 | ≈20 µas (bright) | `GaiaDr3CsvBackend` | `scripts/fetch-gaia-dr3-subset.sh` | `gaia-dr3-i355` |

References: Perryman et al. 1997, A&A 323, L49 (Hipparcos); ESA 1997, *The
Hipparcos and Tycho Catalogues*, ESA SP-1200 (the `V = VT − 0.090(BT−VT)`,
`B−V = 0.850(BT−VT)` Tycho transform); Høg et al. 2000, A&A 355, L27
(Tycho-2); Gaia Collaboration 2022, A&A 674, A1 (Gaia DR3).

The raw archives are **not committed** (Hipparcos ~10 MB, Tycho-2 ~250 MB,
Gaia DR3 ~1.8 billion rows): they are fetched on demand and recorded as
`runtime-service` rows, following the Falchi-atlas precedent. License is
`see-upstream` for the CDS/VizieR catalogues and `CC-BY-SA-3.0-IGO` for Gaia
(use the standard Gaia acknowledgement). Identifier mapping: HIP → primary
`Hipparcos`; Tycho-2 → primary packed `Tycho2` (`(TYC1<<24)|(TYC2<<4)|TYC3`)
with HIP cross-ID; Gaia → primary `GaiaDr3` source_id with optional HIP/HD
cross-IDs. Gaia `G` is used directly as the display magnitude and `BP−RP` is
mapped to an approximate `B−V` for display chroma only (the exact Riello et
al. 2021 transforms are deferred with LOD streaming and host wiring).

### Bright-star HIP↔HD cross-match anchor index (`L-17`)

Repository location: `crates/catalog/data/bright_star_xmatch.csv` (manifest id
`bright-star-xmatch`, `generated`). 177 naked-eye stars (V ≤ 3.0) with their
Hipparcos and Henry Draper numbers, V, and B−V, extracted deterministically
from the in-repo HYG catalogue by `scripts/extract-bright-star-xmatch.py` (no
network dependency). It backs the ingest identifier round-trip tests; the
Tycho-2 / Gaia cross-IDs are filled at ingest time from the catalogue
backends, not hand-entered here. Regenerate with:

```bash
python3 scripts/extract-bright-star-xmatch.py
```

## Constellation data

### Modern western constellation lines

Repository location:

- `crates/renderer/data/constellation_lines.csv`

Used for:

- `OverlayKind::ConstellationLines`;
- renderer-side constellation stick figures.

Manifest id:

- `d3-celestial-constellation-lines` in `data/manifest.toml`.

Source noted in roadmap:

- derived from BSD-licensed d3-celestial line data.

Implementation areas:

- `crates/renderer/build.rs`
- `crates/renderer/src/constellations.rs`
- `crates/renderer/src/overlay.rs`

Maintenance rule:

- If regenerated, document the exact upstream source revision and preprocessing
  command in this file or in the generator script. Update the `sha256` field
  of the manifest entry in the same PR; `make manifest-check` enforces this.

### Messier deep-sky catalog

Repository location:

- `crates/catalog/data/messier.csv`

Manifest id:

- `messier-catalog` in `data/manifest.toml`.

Used for:

- `OverlayKind::DeepSkyObjects` markers and `OverlayKind::DeepSkyLabels` text;
- 110 Messier objects rendered as diamond markers and `M1`, `M31`, ...
  labels, filtered by `OverlayConfig::deep_sky_magnitude_limit`.

Source:

- OpenNGC (`mattiaverga/OpenNGC`) main `NGC.csv`, Messier subset (rows where
  the `M` column is set);
- NGC 2000.0 (Sinnott & Skiff 1988, VizieR VII/118) and SEDS Messier database
  (https://www.messier.seds.org/) backfill for M40 (Winnecke 4), M45
  (Pleiades), and M102 (= NGC 5866) which lack standard NGC ids in OpenNGC;
- M73's missing major-axis size is filled from SEDS (2.8 arcmin).

License:

- The retained columns are factual astronomical numbers (J2000 coordinates,
  V magnitudes, major-axis angular sizes, classifications) and are treated
  as public-domain factual data. OpenNGC is acknowledged as the upstream
  compilation. See `data/manifest.toml` for the manifest entry.

Implementation areas:

- `crates/catalog/build.rs` (emits `messier.bin`);
- `crates/catalog/src/deepsky.rs` (`MessierCatalog` decoder + tests);
- `crates/renderer/build.rs` (bakes the `M1`..`M110` label-position table);
- `crates/renderer/src/overlay.rs` (4-segment diamond marker);
- `crates/renderer/src/text.rs` (label rendering).

Maintenance rules:

- The build script asserts every Messier number 1…110 appears exactly once;
  removing or duplicating a row fails the build.
- M40 and M102 use the standard published identifications (Winnecke 4 and
  NGC 5866 respectively). Changing those identifications must update
  `DATA_SOURCES.md`, the CSV, and the manifest hash together.
- Magnitudes are V-band single-value approximations; for extended objects
  like M31 / M42 / M45 no single number describes the brightness
  distribution well. Treat the values as catalogue-grade ordering, not
  photometric ground truth.

### Bright NGC / IC deep-sky subset

Repository location:

- `crates/catalog/data/openngc_bright.csv`

Manifest id:

- `openngc-bright-catalog` in `data/manifest.toml`.

Used for:

- `OverlayKind::DeepSkyObjects` markers and `OverlayKind::DeepSkyLabels` text;
- ~1,250 NGC / IC objects rendered as 8-segment ring markers (distinct from
  the Messier diamond shape) and `NGC7000`, `IC434`, … labels, filtered by
  the same `OverlayConfig::deep_sky_magnitude_limit` slider as the Messier
  overlay.

Source:

- OpenNGC (`mattiaverga/OpenNGC`) main `NGC.csv` + `addendum.csv`.
  Filter (`scripts/extract-openngc-bright.py`):
  - exclude Messier rows (covered by `messier-catalog`);
  - exclude OpenNGC type codes `*`, `**`, `*Ass`, `NonEx`, `Other`;
  - keep `min(V-Mag, B-Mag - 0.6) ≤ 11.5 mag` when either band is
    published;
  - keep emission / reflection / diffuse nebulae (`EmN`, `RfN`, `Neb`,
    `HII`, `Cl+N`, `SNR`) with major axis ≥ 30 arcmin even when no
    integrated photometry is published (sentinel magnitude = 99.00 so the
    default density slider hides them).

Citations:

- Dreyer, J. L. E. 1888, MmRAS 49, 1 (original NGC);
- Dreyer, J. L. E. 1908, MmRAS 59, 105 (IC);
- Verga, M. (current), OpenNGC repository — modernised compilation.

License:

- Retained columns are factual numerical data (J2000 coordinates, V-band
  magnitudes, major-axis sizes, classifications) treated as public-domain
  factual values; OpenNGC is acknowledged as the upstream compilation.

Implementation areas:

- `scripts/extract-openngc-bright.py` (deterministic regenerator);
- `crates/catalog/build.rs` (emits `openngc_bright.bin`);
- `crates/catalog/src/deepsky.rs` (`NgcBrightCatalog` decoder + tests);
- `crates/renderer/build.rs` (bakes the NGC / IC label-position table);
- `crates/renderer/src/overlay.rs` (8-segment ring marker);
- `crates/renderer/src/text.rs` (label rendering).

Maintenance rules:

- `scripts/extract-openngc-bright.py` is the single source of truth for the
  committed CSV. Re-run with `--ngc <path>` / `--addendum <path>` against a
  cached OpenNGC snapshot to get byte-stable output, then update the
  manifest `sha256` and `bytes` fields together.
- The committed subset is documented to miss a small number of famous
  diffuse objects that OpenNGC marks as duplicates or lacks size /
  photometry for (NGC 2244 Rosette cluster, IC 1396, IC 2118). The planned
  runtime streaming backend in the V-42 follow-up PR will expose those
  entries via the full OpenNGC catalogue without re-baking this subset.
- Magnitudes are single-value approximations; for large emission nebulae
  with sentinel magnitudes (e.g. NGC 7000, NGC 2237) the slider must be
  opened past the sentinel to surface them.
- Object names must follow `NGC<n>` or `IC<n>` form (suffix letters
  allowed). Other OpenNGC designations (Caldwell, ESO, Melotte) are
  skipped by the extraction script and belong to the runtime backend.

### Open-cluster membership (showpiece bootstrap)

Repository location:

- `crates/catalog/data/cluster_membership.csv`

Used for:

- `crates/catalog::clusters` (open-cluster membership lookup);
- `DeepSkyCatalog::resolve_as_member_field` (V-53 marker suppression).

Manifest id:

- `open-cluster-membership-bootstrap` in `data/manifest.toml`.

Source:

- Cantat-Gaudin, T. et al. 2020, A&A 633, A99 — VizieR catalogue
  `J/A+A/633/A99`, "Painting a portrait of the Galactic disc with its
  stellar clusters" (DOI 10.1051/0004-6361/201936691). Listed as the
  upstream the follow-up extraction will pull from.
- Mermilliod, J.-C. & Paunzen, E. 2003, A&A 410, 511 — WEBDA database
  context for showpiece-cluster membership lists.

License:

- CC-BY-4.0 (CDS / VizieR redistribution terms apply); see
  `https://cds.u-strasbg.fr/vizier/Documents/license.htx`.

Implementation areas:

- `crates/catalog/src/clusters.rs` (parser + lookup APIs);
- `crates/catalog/src/deepsky.rs`
  (`DeepSkyCatalog::resolve_as_member_field` trait method);
- `crates/renderer/src/overlay.rs` (suppresses tagged cluster markers);
- `scripts/extract-cluster-membership.py` (deterministic regenerator).

V-53 first slice (hand-curated showpiece bootstrap):

- Pleiades (M45): 9 named-star members.
- Praesepe / Beehive (M44): 11 brightest core members at V ≤ 6.9.
- Double Cluster (NGC 869 + NGC 884): the HYG-resolvable bright members
  split by RA (RA < 2.345 → NGC 869, RA ≥ 2.345 → NGC 884).
- Hyades (Mel 25) is intentionally deferred to the Cantat-Gaudin
  follow-up; it has no current V-42 DSO marker to suppress.

Maintenance rules:

- Re-running `python scripts/extract-cluster-membership.py` must produce
  the committed CSV byte-identically; the bootstrap row list lives in
  the script and is the single source of truth.
- The `cluster_id` column must always resolve to a current V-42 DSO id
  (`M<N>`, `NGC<N>`, `IC<N>`); rows that use other designations
  (`Mel<N>`, `Cr<N>`) are kept in the CSV for the follow-up but the
  parser drops them.
- The follow-up Cantat-Gaudin extraction lives behind
  `scripts/extract-cluster-membership.py --from-cantat-gaudin` and must
  keep the same column shape so the manifest re-hash works without
  schema churn.

### Variable-star light-curve elements (`L-20`)

Repository location:

- `crates/catalog/data/variable_stars.csv`

Used for:

- `crates/catalog::variables` (light-curve element lookup + phase-folded
  magnitude prediction), surfaced through `GotoTarget` (CLI `--goto` Δm
  metadata) and `goto_object` (web info-panel light curve).

Manifest id:

- `vsx-variable-stars-bootstrap` in `data/manifest.toml`.

Source:

- AAVSO International Variable Star Index (VSX; Watson, C. L. 2006, SASS 25,
  47), `https://www.aavso.org/vsx/`, and the General Catalogue of Variable
  Stars (GCVS 5.1; Samus' et al. 2017, ARep 61, 80). Type, period `P`, epoch
  `T0`, and bright/faint V magnitudes for six showpiece variables (Mira,
  Algol, delta Cephei, Betelgeuse, chi Cygni, RR Lyrae). Hand-curated from the
  published elements.

License:

- See upstream (AAVSO VSX / GCVS). Elements are factual catalogue data.

Preprocessing / limitations:

- Hand-curated (no script). The predicted magnitude is a first-order *visual*
  model: a smoothed raised-cosine pulsation (maximum at phase 0, minimum at
  0.5) for Mira / semiregular / Cepheid / RR Lyrae stars, and a raised-cosine
  primary + shallow secondary eclipse for Algol-type (EA) binaries. It is not a
  Fourier light-curve fit, so the predicted *shape* (asymmetric Cepheid /
  RR Lyrae rise, true Mira amplitude variation) is illustrative; the
  magnitude *range* and *phase* are accurate to the elements. Semiregular
  periods (Betelgeuse) are not strictly periodic. Epochs are treated as plain
  JD (HJD / light-time differences ≤ ~8 min are negligible against the
  multi-day to multi-hundred-day periods).

### Washington Double Star showpiece table (`V-54`)

Repository location:

- `crates/catalog/data/double_stars.csv`

Used for:

- `crates/catalog::doubles` (visual double / binary resolution lookup);
- `crates/catalog::resolve_doubles` (catalog-load-time component split,
  applied on both the HYG-CSV and embedded paths).

Manifest id:

- `wds-double-stars-bootstrap` in `data/manifest.toml`.

Source:

- Mason, B. D., Wycoff, G. L., Hartkopf, W. I., Douglass, G. G.,
  Worley, C. E. 2001, AJ 122, 3466, "The 2001 US Naval Observatory
  Double Star CD-ROM. I. The Washington Double Star Catalog"
  (DOI 10.1086/323920); USNO-maintained updates,
  `http://www.astro.gsu.edu/wds/`. Separation `ρ`, position angle `θ`,
  and component magnitudes per epoch.

License:

- Public domain (US Naval Observatory). See
  `http://www.astro.gsu.edu/wds/`.

Implementation areas:

- `crates/catalog/src/doubles.rs` (parser + `resolve_doubles`);
- `crates/catalog/src/catalog.rs` (both load paths call `resolve_doubles`);
- `scripts/extract-double-stars.py` (deterministic regenerator).

V-54 first slice (hand-curated showpiece bootstrap) — the visual doubles
HYG v4.2 merges into one row:

- Algieba (γ Leo, HYG id 50440): golden K-type A/B, ρ = 4.6″, θ = 126°.
- ε Lyrae "Double Double" (HYG ids 91633 / 91639): each row is itself an
  unresolved pair, so resolution yields four sprites total.

Pairs HYG already ships as two distinct rows are intentionally omitted to
avoid double-counting: Albireo (β1 / β2 Cyg), Castor (α Gem A/B), and
Mizar (A = id 65173, B = id 118887 at ~19″, with Alcor = id 65272 at
~12′).

Maintenance rules:

- Re-running `python scripts/extract-double-stars.py` must produce the
  committed CSV byte-identically; the bootstrap row list lives in the
  script and is the single source of truth.
- Only add a pair here if HYG merges its components into a single row;
  pairs HYG already resolves must stay out to avoid a phantom third
  component.
- The full-catalog WDS extraction lives behind
  `scripts/extract-double-stars.py --from-wds` (stubbed) and must keep
  the same column shape so the manifest re-hash works without schema
  churn.

### IAU / Delporte constellation boundaries

Repository location:

- `crates/renderer/data/constellation_boundaries.csv`

Used for:

- `OverlayKind::ConstellationBoundaries`;
- IAU/Delporte sky-region boundaries.

Manifest id:

- `iau-delporte-constellation-boundaries` in `data/manifest.toml`.

Source noted in roadmap:

- CDS VI/49 / Delporte 1930 B1875 boundary vertices;
- vertices are precessed to J2000 for renderer use.

Implementation areas:

- `crates/renderer/build.rs`
- `crates/renderer/src/constellations.rs`
- `crates/renderer/src/overlay.rs`

Maintenance rule:

- Keep the coordinate epoch and preprocessing method explicit. Boundary data is
  easy to misuse if B1875 and J2000 coordinates are mixed.

## Artificial-satellite orbital elements (V-55)

### Curated CelesTrak TLE snapshot

Repository location:

- `crates/common/data/satellites/curated_tle.txt`

Used for:

- the V-55 artificial-satellite layer (TLE / SGP4) in `astronomy::satellites`,
  embedded via `include_str!` into `crates/common/src/satellites.rs` and
  `apps/web/src/lib.rs`.

Manifest id:

- `celestrak-tle-curated-2026-05` in `data/manifest.toml`.

Source / version / license:

- CelesTrak GP service (`https://celestrak.org/NORAD/elements/gp.php`),
  retrieved 2026-05-31 (TLE epoch ~2026-05-30, day-of-year 150).
- A curated representative set: ISS (ZARYA), HST, NOAA-20 (polar LEO),
  STARLINK-1008, and the geostationary GOES-16.
- License: US Space Force orbital data redistributed by CelesTrak is in the
  public domain.

References:

- Vallado, D. A. et al. 2006, AIAA 2006-6753, *Revisiting Spacetrack Report #3*
  (SGP4 reference implementation, matched by the `sgp4` crate).
- Hoots, F. R. & Roehrich, R. L. 1980, Spacetrack Report #3.

Preprocessing / regeneration:

- `scripts/fetch-satellite-tle.sh` re-fetches the snapshot from CelesTrak.
  After regenerating, refresh the `sha256` + `retrieved` date in
  `data/manifest.toml` and this file, then run `make manifest-check`.

Intrinsic magnitudes:

- Per-satellite intrinsic (“standard”) visual magnitudes are a small
  hand-curated table in `crates/astronomy/src/satellites.rs`
  (`CURATED_STD_MAGNITUDES`), following the McCants / QuickSat convention
  (mmccants.org) — the V magnitude at 1000 km range and 50 % illuminated
  phase. This is deliberately a small hand table for the curated set, not a
  bulk import of McCants’ MCNAMES file.

Maintenance rule:

- TLEs are only accurate near their epoch (SGP4 drifts over weeks). The
  snapshot is for deterministic demonstration / validation of the SGP4
  pipeline, **not** operational tracking. Live TLE fetch is an opt-in host
  concern only (see `docs/standards-compliance.md`); the default render path
  uses this pinned snapshot so renders stay reproducible.

Implementation areas:

- `crates/astronomy/src/satellites.rs`
- `crates/common/src/satellites.rs`
- `crates/renderer/src/camera.rs`, `crates/renderer/src/shaders/skyglow.wgsl`

## Optional external dataset: Falchi 2016 World Atlas (V-39-Atlas)

Upstream source (not committed):

- Falchi, F. et al. 2016, *The new world atlas of artificial night sky
  brightness*, **Science Advances** 2, e1600377 (doi:10.1126/sciadv.1600377).
- 2015 data release (GeoTIFF) via GFZ Data Services,
  doi:10.5880/GFZ.1.4.2016.001.

Manifest id:

- `falchi-2016-world-atlas` in `data/manifest.toml` (kind = `runtime-service`;
  it has no committed local bytes because the raster is ~1 GB).

Field used:

- band 1 — the ratio of artificial zenith sky brightness to the natural
  background (Falchi's adopted natural reference ≈ 0.174 mcd/m², V ≈ 21.6
  mag/arcsec²).

License / terms:

- recorded as `see-upstream`; accept the release terms at the DOI landing
  page before downloading. Only the user's own machine fetches the file.

Preprocessing / local storage:

1. `scripts/fetch-falchi-atlas.sh` downloads the GeoTIFF (URL supplied via
   `FALCHI_ATLAS_URL`, since the direct link sits behind the DOI page).
2. `scripts/build-falchi-atlas.py` block-averages it to a coarse regular
   lat/lng grid and converts each cell's ratio `r` to a total zenith V-band
   surface brightness with the flux-additive model
   `μ = 21.6 − 2.5·log10(1 + r)` (matching the renderer's natural floor),
   writing the compact little-endian `FALATL01` grid documented in
   `crates/astronomy/src/light_pollution_atlas.rs`.
3. Point the native hosts at the result with `STARS_FALCHI_ATLAS=<path>`.

Implementation areas:

- `crates/astronomy/src/light_pollution_atlas.rs` — `FalchiAtlas` parser +
  bilinear `sample_zenith_mag_per_arcsec2` (IO-free, unit-tested with a
  synthetic grid fixture);
- `crates/common/src/lib.rs` — `load_falchi_atlas` / `resolve_light_pollution`
  read `STARS_FALCHI_ATLAS` and map `LightPollution::Atlas2016` to a sampled
  `LightPollution::Sqm`; `apps/cli` / `apps/viewer` resolve at render time.

Determinism / fallback:

- the default render path never reads the atlas; when `STARS_FALCHI_ATLAS` is
  unset (or a location is outside coverage) `Atlas2016` keeps the rural
  Bortle-1 floor, so committed scene presets stay byte-stable.

## Comet orbital elements (V-49)

### Curated JPL SBDB osculating-element snapshot

Repository location:

- `crates/common/data/comets/elements.csv`

Used for:

- the V-49 comet layer (two-body Keplerian propagation + coma / tail
  rendering) in `astronomy::comets`, embedded via `include_str!` into
  `crates/common/src/comets.rs` and `apps/web/src/lib.rs`.

Manifest id:

- `jpl-sbdb-comet-elements-2025-01` in `data/manifest.toml`.

Source / version / license:

- JPL Small-Body Database (`https://ssd.jpl.nasa.gov/sbdb.cgi`), retrieved
  2025-01 for the listed objects. Heliocentric osculating Keplerian elements
  (`q`, `e`, `i`, `ω`, `Ω`, `Tp`) referred to the J2000.0 ecliptic in the
  Marsden / Minor Planet Center convention.
- A curated representative set: 1P/Halley (1986 apparition), C/1995 O1
  (Hale-Bopp), and C/2023 A3 (Tsuchinshan-ATLAS).
- License: NASA/JPL SSD/CNEOS data is in the public domain.

References:

- Finson, M. L. & Probstein, R. F. 1968, ApJ 154, 327 (dust-tail dynamics).
- Marsden, B. G. & Williams, G. V., MPC orbital-element format.
- Bobrovnikoff, N. T. 1942, ApJ 95, 71; Bowell, E. et al. 1989 (comet
  magnitude-law conventions).

Magnitude-law coefficients:

- Each row carries representative Bobrovnikoff-Bowell `(M1, K1)` total-magnitude
  coefficients (`m1 = M1 + 5 log₁₀ Δ + K1 log₁₀ r`) for naked-eye rendering —
  not authoritative photometry.

Maintenance rule:

- Two-body propagation from a single osculating-element set is accurate only
  near each element epoch (planetary perturbations and the N-body upgrade are
  tracked under `L-06`). The snapshot is for deterministic demonstration /
  validation, not precision ephemerides. Refresh the `sha256` + `retrieved`
  date in `data/manifest.toml` and this file when updating, then run
  `make manifest-check`.

Implementation areas:

- `crates/astronomy/src/comets.rs`
- `crates/common/src/comets.rs`
- `crates/renderer/src/camera.rs`, `crates/renderer/src/shaders/skyglow.wgsl`

## Runtime web services

### OpenStreetMap Nominatim search API

Runtime endpoint:

- `https://nominatim.openstreetmap.org/search`

Manifest id:

- `openstreetmap-nominatim` in `data/manifest.toml` (kind = `runtime-service`).

Used for:

- browser-only address / place-name lookup in the web location panel;
- converting a typed address into observer latitude / longitude.

License / terms:

- OpenStreetMap data is available under the Open Database License (ODbL);
- Nominatim public API usage is subject to the OpenStreetMap Foundation
  Nominatim usage policy.

Implementation areas:

- `apps/web/frontend/src/components/StatusBar.tsx`

Preprocessing / local storage:

- none; results are fetched at runtime and only the selected coordinates are
  applied to the current browser session state.

## Literature-derived model inputs

The roadmap names the primary references for implemented physical and
astronomical models. Important examples include:

### Photometry and human vision

Used for:

- magnitude to illuminance;
- mesopic chromatic-fidelity weighting;
- scotopic desaturation;
- rod/cone tone response;
- glare / PSF.

References named in roadmap:

- Schaefer, B. E. 1990, PASP 102, 212;
- CIE 191:2010;
- CIE 1951 V'(λ);
- Bowmaker & Dartnall 1980;
- Spencer, Shirley, Zimmerman & Greenberg 1995;
- Ritschel et al. 2009;
- Ferwerda et al. 1996;
- Reinhard et al. 2002;
- Pattanaik et al. 1998;
- Durand & Dorsey 2002;
- Ballesteros 2012.

Implementation areas:

- `crates/astronomy/src/photometry.rs`
- `crates/catalog/src/color.rs`
- `crates/renderer/src/tonemap.rs`
- renderer shaders.

### Atmosphere, skyglow, and extinction

Used for:

- airmass;
- extinction;
- diffuse sky background;
- zodiacal light;
- airglow;
- dust extinction;
- daylight / twilight sky colour.

References named in roadmap:

- Kasten & Young 1989;
- Hardie 1962;
- Schaefer 1993;
- Leinert et al. 1998;
- Roach & Megill 1961;
- Schlegel, Finkbeiner & Davis 1998;
- Hošek & Wilkie 2012 (V-38 default daylight sky-dome model);
- Krisciunas & Schaefer 1991;
- Bortle, J. E. 2001, S&T 101(2), 126 — Bortle Dark-Sky Scale class →
  zenith SQM table (V-39 core);
- Cinzano, P., Falchi, F. & Elvidge, C. D. 2001, MNRAS 328, 689 —
  long-form artificial-sky-glow scattering model the V-39 core simplifies;
- Garstang, R. H. 1986, PASP 98, 364 — single-scattering zenith-distance
  kernel (V-39 core);
- Falchi, F. et al. 2016, *Science Advances* 2, e1600377 — World Atlas
  GeoTIFF source for the `V-39-Atlas` loader (see the dedicated section above);
- ASTM G-173 / CIE daylight-basis references where used by code comments.

Implementation areas:

- `crates/astronomy/src/atmosphere.rs`
- `crates/astronomy/src/atmosphere/hosek_wilkie.rs`
- `crates/astronomy/src/illuminants.rs`
- `crates/astronomy/src/skyglow.rs`
- `crates/renderer/src/skyglow.rs`
- renderer shaders.

Embedded Hošek-Wilkie coefficient table (V-38):

- Source: <https://cgg.mff.cuni.cz/projects/SkylightModelling/>,
  release v1.4a (22 Feb 2013), BSD 3-clause.
- Local path: `crates/astronomy/data/hosek_wilkie/coefficients_rgb.bin`
  (28,816 bytes; hashed in `data/manifest.toml` under
  `hosek-wilkie-2012-rgb-v1.4a`).
- Regenerator: `scripts/build-hosek-wilkie.py`. Re-running the script
  re-downloads the upstream `ArHosekSkyModelData_RGB.h` and rewrites the
  packed binary; `make manifest-check` then fails until the SHA-256 is
  re-pinned in the manifest.
- Fields used: `datasetRGB{1,2,3}` (9 polynomial coefficients per
  channel / albedo / turbidity / elevation control point) and
  `datasetRGBRad{1,2,3}` (per-channel master radiance scale). The
  spectral and CIE XYZ tables shipped with the same upstream archive are
  not vendored — only the RGB path is consumed by the renderer.

### Meteor showers (V-47)

Used for the V-47 meteor-shower display (radiant-based streaks with
date-dependent rate). The IMO Working List of Visual Meteor Showers is
transcribed into a small constant table
(`astronomy::meteors::IMO_WORKING_LIST`: Quadrantids, Lyrids, eta Aquariids,
Perseids, Orionids, Leonids, Geminids, Ursids) carrying each shower's J2000
radiant α/δ at maximum, peak solar longitude, maximum ZHR, population index
`r`, geocentric velocity `v∞`, and a solar-longitude activity slope `B`.

This is a literature-derived constant table embedded in Rust source (not a
committed data artifact), so there is **no `data/manifest.toml` row** — the
same treatment as the V-55 per-satellite standard-magnitude table. The
constants are transcribed from:

- Koschack, R. & Rendtel, J. 1990, WGN 18, 44 — visual ZHR reduction
  `ZHR = n·F·r^(6.5−lm)/sin h_R`; the renderer samples the inverse
  observed-rate form `n = ZHR·sin h_R·r^(lm−6.5)/F`;
- Jenniskens, P. 1994, A&A 287, 990 — annual-stream activity profiles
  (ZHR_max, population index, solar-longitude slope `B`);
- IMO Meteor Shower Calendar (Rendtel et al., annual) — radiant positions,
  peak solar longitudes, and ZHR for the working-list showers;
- McKinley, D. W. R. 1961, *Meteor Science and Engineering* — entry-velocity
  / trail-geometry background.

Implementation area: `crates/astronomy/src/meteors.rs`;
rendered via `crates/renderer/src/camera.rs` (`MeteorLayer`,
`meteor_uniforms`) and `crates/renderer/src/shaders/skyglow.wgsl`
(`meteor_radiance`).

### Time, coordinate corrections, and ephemerides

Used for:

- time scales;
- precession;
- nutation;
- annual aberration;
- atmospheric refraction;
- apparent Sun / Moon / planet positions;
- apparent Galilean-moon positions and magnitudes (V-52b);
- apparent Titan position and magnitude (V-52c).

References / standards named in roadmap:

- IAU 2006 precession / P03 Fukushima-Williams matrix;
- compact IAU-2000-style luni-solar nutation terms;
- Saemundsson 1986 refraction style;
- VSOP87 / FK5 Sun approximation;
- ELP2000-style Moon approximation;
- WGS84 topocentric parallax;
- Lainey, V., Duriez, L., Vienne, A. 2006, A&A 456, 783 — L1.2
  semi-analytic theory of the Galilean satellites (V-52b-E5, the
  pivoted target that replaced the originally-planned Lieske 1998 E5
  source because the L1.2 IMCCE distribution is the only reachable
  machine-readable table at equivalent accuracy). Coefficient table
  `BisL1.2.dat` is embedded into the `astronomy` crate via
  `include_str!` and pinned in `data/manifest.toml` as
  `lainey-2006-l12-galilean-coeffs`. Reduced `V(1, 0)` magnitudes for
  the four moons are still taken from Meeus 1998 table 41.A;
- Lieske 1998, A&AS 129, 205 — kept for citation completeness;
  superseded operationally by Lainey 2006 L1.2;
- Meeus 1998 *Astronomical Algorithms* ch. 44 — still used by the
  V-52d shadow producer (`jupiter_shadows.rs`) until the follow-up
  rung `V-52d-L1.2` ports the shadow projection onto L1.2 too;
- JPL Horizons On-Line Ephemeris System
  (https://ssd.jpl.nasa.gov/horizons/) — geocentric ICRF apparent
  RA / Dec / range pinned for V-52b-E5 in
  `data/horizons_galilean_moons.csv` (regenerated by
  `scripts/fetch-horizons-galilean-moons.sh`); the same fixture
  pins the L1.2 ≤20″ accuracy gate;
- Vienne & Duriez 1995 A&A 297, 588 — TASS1.7 — full semi-analytic
  theory of Titan's motion (`V-52c-TASS17`, the operational Titan
  ephemeris; supersedes the Meeus 1998 ch. 45 reduction `V-52c`
  shipped). The IMCCE `tass17.f` series block is extracted to
  `crates/astronomy/data/redtass7.dat` (by `scripts/build-tass17.sh`),
  embedded into the `astronomy` crate via `include_str!` from
  `crates/astronomy/src/moons/tass17.rs`, and pinned in
  `data/manifest.toml` as `vienne-duriez-1995-tass17-titan-coeffs`. The
  port is validated bit-for-bit against the IMCCE `EXAMP7.res`
  reference positions (<1e-10 AU);
- JPL Horizons On-Line Ephemeris System
  (https://ssd.jpl.nasa.gov/horizons/) — geocentric ICRF apparent
  RA / Dec / range for Saturn + Titan pinned for V-52c-TASS17 in
  `data/horizons_titan.csv` (regenerated by
  `scripts/fetch-horizons-titan.sh`); the apparent Titan-vs-Saturn
  offset matches this fixture to ≈0.1″ at J2000 and ≈3–4″ at the
  ±100-yr extremes;
- Karkoschka 1998 *Icarus* 133, 134 for Titan visual photometry
  (source of the `V(1, 0) = −1.28` reduced magnitude used in V-52c);
- Archinal et al. 2018 CMDA 130, 22 for Galilean / Titan physical
  radii and IAU WGCCRE rotation parameters.

Implementation areas:

- `crates/astronomy/src/time.rs`
- `crates/astronomy/src/corrections.rs`
- `crates/astronomy/src/ephemeris.rs`
- `crates/astronomy/src/moons.rs` (V-52b Galilean satellites, V-52c Titan)
- `crates/astronomy/src/meteors.rs` (V-47 meteor showers)
- `crates/astronomy/src/moons/lainey_l1.rs` (V-52b-E5 — full Lainey
  2006 L1.2 series + IMCCE coefficient parser; replaces the prior
  Lieske 1998 E5 scaffold)
- `crates/astronomy/data/BisL1.2.dat` (embedded L1.2 coefficient table)
- `crates/astronomy/src/moons/tass17.rs` (V-52c-TASS17 — full
  Vienne & Duriez 1995 TASS1.7 Titan series + IMCCE series parser)
- `crates/astronomy/data/redtass7.dat` (embedded TASS1.7 series table)
- `crates/astronomy/src/observer.rs`
- `crates/renderer/src/camera.rs`

## Generated / embedded data

Some data is transformed at build time or embedded for WASM / single-binary use.

Current generated / embedded paths (every committed regenerable artifact also
has an entry in `data/manifest.toml` so `make manifest-check` can detect
unaccounted-for regeneration):

- `crates/catalog/build.rs` for embedded catalog support;
- `crates/renderer/build.rs` for compact constellation data and generated
  label metadata (`label_data.rs` in Cargo `OUT_DIR`);
- `docs/presets/sessions/*.json`, generated by
  `scripts/export-scene-presets.sh` from `--list-presets` (manifest ids
  `scene-preset-session-*`);
- `docs/assets/readme/*.png`, generated by
  `scripts/generate-readme-images.sh` from deterministic CLI scenes using the
  local HYG catalog and renderer defaults (manifest ids `readme-image-*`);
- `docs/assets/validation/*.png`, generated by
  `scripts/render-validation-gallery.sh` (manifest ids
  `validation-gallery-*`). Byte-exact re-render comparison is opt-in via
  `--check`; the manifest hash still pins the committed bytes;
- `docs/assets/demo-gallery/*.png`, generated by
  `scripts/render-demo-gallery.sh` (manifest ids `demo-gallery-*`).
  Curated subset of the validation presets surfaced through
  [`docs/demo-gallery.md`](docs/demo-gallery.md) as the project front-
  door showcase (L-14). Byte-exact re-render comparison via
  `--check` (or `make demo-gallery-check`) is opt-in; the manifest
  hash pins the committed bytes the same way the validation gallery
  does;
- `examples/notebooks/expected/*-session-table.csv`, generated by
  `cargo run -q -p stars-cli --example session-table -- <session.json>` from
  committed JSON sessions and current `crates/astronomy` Sun/Moon/planet
  apparent-position models (manifest ids `notebook-expected-*`). These
  fixtures are checked in so notebook examples and `make notebook-check` can
  review numerical drift without Jupyter, a star catalog, or a GPU.
- `data/horizons_galilean_moons.csv`, generated by
  `scripts/fetch-horizons-galilean-moons.sh` from the public JPL
  Horizons On-Line Ephemeris System API (manifest id
  `horizons-galilean-moons-fixture`). Geocentric ICRF apparent
  positions for Jupiter + the four Galilean moons at three epochs
  spanning ±100 years (1900-01-01, 2000-01-01, 2100-01-01 UT). Pinned
  validation target for the V-52b-E5 (Lainey L1.2) precision upgrade —
  the test `moons::tests::galilean_matches_horizons_within_l1_budget`
  enforces the ≤20″ tolerance band against every fixture epoch.
- `crates/astronomy/data/BisL1.2.dat`, fetched from IMCCE
  (`ftp://ftp.imcce.fr/pub/ephem/satel/galilean/L1/L1.2/BisL1.2.dat`)
  and embedded into the `astronomy` crate at compile time via
  `include_str!` (manifest id `lainey-2006-l12-galilean-coeffs`,
  kind `embedded`). Coefficient table backing the V-52b-E5
  Galilean-moon precision upgrade (Lainey 2006 L1.2).
- `data/horizons_titan.csv`, generated by
  `scripts/fetch-horizons-titan.sh` from the same public JPL Horizons
  API (manifest id `horizons-titan-fixture`). Geocentric ICRF apparent
  positions for Saturn + Titan at the same three epochs. Pinned
  validation target for the V-52c-TASS17 precision upgrade — the test
  `moons::tests::titan_matches_horizons_within_tass17_budget`
  enforces the *current* (Meeus-grade) Kronocentric tolerance band
  against it so the precision upgrade can tighten the bound by editing
  one constant.

Rules:

1. Generated output should be deterministic.
2. The source file and transform should be documented.
3. If generated data is checked in, explain why.
4. If generated data changes rendering or numerical output, add or update tests.
5. If generated data is committed, add an entry to `data/manifest.toml` with
   `kind = "generated"`, the SHA-256, and the regeneration command in
   `preprocessing`.

## DE440 ephemeris kernel (`L-06`) — external, not committed

`stars` ships a DAF/SPK Chebyshev kernel reader
(`crates/astronomy/src/spk.rs`, `astronomy::SpkKernel`) capable of loading
JPL DE440 / DE441 binary SPK kernels and evaluating Type 2 / Type 3
Chebyshev segments. **No DE440 kernel is committed to this repository**, so
there is no `data/manifest.toml` row for it: the kernels are large binaries
(`de440s.bsp` ≈ 32 MB for 1849–2150; full `de440.bsp` ≈ 110 MB) and the
default / WASM build deliberately keeps the analytic VSOP87 / ELP2000
fallback (`astronomy::ephemeris`).

- **Source / archive:** NASA NAIF generic kernels,
  `https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/de440s.bsp`
  (and `de440.bsp`).
- **Version / release:** DE440 (Park et al. 2021, AJ 161, 105); NAIF SPK
  format per Acton 1996.
- **License / terms:** U.S. Government work / NAIF public domain; redistribute
  per NAIF terms. Because it is not redistributed here, no license obligation
  is incurred by this repo.
- **Local path:** user-supplied at runtime (e.g. `build/de440/de440s.bsp`); not
  tracked by git.
- **Retrieval command:** `scripts/fetch-de440-subset.sh [OUT_BSP]`.
- **Fields used:** geocentric / barycentric Chebyshev position (Type 2) and
  position+velocity (Type 3) for the Sun, Moon, and planet barycenters.
- **Known limitations / status:** the reader is implemented and unit-tested
  against a synthetic, spec-accurate in-memory SPK. Wiring a fetched DE440
  kernel into the apparent-place pipeline and the JPL Horizons sub-arcsecond
  cross-check are **deferred** (they require the external kernel); see
  `ROADMAP.md` `L-06`. Until then the renderer's Sun/Moon/planet output is the
  VSOP87 / ELP2000 visual tier.

## Future data sources to document

When these roadmap items are implemented, add details here **and** append a
row to `data/manifest.toml`:

- Hipparcos catalog;
- Tycho-2 catalog;
- Gaia DR3 catalog;
- AAVSO variable-star light curves;
- full OpenNGC ~14,000-entry NGC / IC catalog (runtime-loaded streaming
  backend, the V-42 follow-up to the shipped bright subset);
- telescope / eyepiece preset data;
- curated public demo-gallery session files;
- large-catalog spatial indexes, LOD subsets, or WASM-specific extracts.

For each future source, record:

- exact source URL or archive identifier;
- version / release date;
- license and redistribution terms;
- local path;
- preprocessing command;
- fields used;
- known limitations.
