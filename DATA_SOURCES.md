# Data sources

This document records external data and literature-derived inputs used by
`stars`. Keep it updated whenever adding catalog files, generated datasets,
shader lookup data, scene presets, validation-gallery inputs, telescope presets,
or coefficients copied from a source.

## Machine-readable provenance manifest

Planned Phase 3 work should add a deterministic manifest, for example
`data/manifest.json` or per-crate manifests, that records every data artifact in
a machine-readable form. The manifest should include:

- exact source URL, archive identifier, or literature citation;
- version / release date and retrieval date;
- license and redistribution terms;
- local path and content hash;
- preprocessing command or generator version;
- fields used by `stars`;
- known limitations and whether the artifact is embedded, generated, or fetched
  at runtime.

`DATA_SOURCES.md` remains the human-readable narrative. The manifest is the
review and reproducibility hook used by JSON sessions, gallery scenes, and large
catalog backends.

## Star catalog

### HYG Database v4.2

Repository location:

- `crates/catalog/data/hyg_v42.csv`

Used for:

- embedded and filesystem star catalog loading;
- star positions;
- magnitudes;
- B−V colour values;
- proper motion where available;
- distance filtering;
- generated top-50 bright-star labels and bright-star-weighted constellation
  label anchors in `crates/renderer/build.rs`.

Acquisition:

```bash
./scripts/download-catalog.sh
```

Implementation areas:

- `crates/catalog/src/catalog.rs`
- `crates/catalog/src/coords.rs`
- `crates/catalog/src/color.rs`
- `crates/catalog/build.rs`

Current filtering policy:

- rows fainter than magnitude 8 are dropped;
- rows with HYG's `100000` parsec sentinel for unknown parallax are dropped.

Notes:

- If a future Gaia / Hipparcos / Tycho backend lands, record its source,
  license, version, preprocessing, and identifier mapping here.

## Constellation data

### Modern western constellation lines

Repository location:

- `crates/renderer/data/constellation_lines.csv`

Used for:

- `OverlayKind::ConstellationLines`;
- renderer-side constellation stick figures.

Source noted in roadmap:

- derived from BSD-licensed d3-celestial line data.

Implementation areas:

- `crates/renderer/build.rs`
- `crates/renderer/src/constellations.rs`
- `crates/renderer/src/overlay.rs`

Maintenance rule:

- If regenerated, document the exact upstream source revision and preprocessing
  command in this file or in the generator script.

### IAU / Delporte constellation boundaries

Repository location:

- `crates/renderer/data/constellation_boundaries.csv`

Used for:

- `OverlayKind::ConstellationBoundaries`;
- IAU/Delporte sky-region boundaries.

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

## Runtime web services

### OpenStreetMap Nominatim search API

Runtime endpoint:

- `https://nominatim.openstreetmap.org/search`

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
- Preetham, Shirley & Smits 1999;
- Krisciunas & Schaefer 1991;
- ASTM G-173 / CIE daylight-basis references where used by code comments.

Implementation areas:

- `crates/astronomy/src/atmosphere.rs`
- `crates/astronomy/src/illuminants.rs`
- `crates/astronomy/src/skyglow.rs`
- `crates/renderer/src/skyglow.rs`
- renderer shaders.

### Time, coordinate corrections, and ephemerides

Used for:

- time scales;
- precession;
- nutation;
- annual aberration;
- atmospheric refraction;
- apparent Sun / Moon / planet positions.

References / standards named in roadmap:

- IAU 2006 precession / P03 Fukushima-Williams matrix;
- compact IAU-2000-style luni-solar nutation terms;
- Saemundsson 1986 refraction style;
- VSOP87 / FK5 Sun approximation;
- ELP2000-style Moon approximation;
- WGS84 topocentric parallax.

Implementation areas:

- `crates/astronomy/src/time.rs`
- `crates/astronomy/src/corrections.rs`
- `crates/astronomy/src/ephemeris.rs`
- `crates/astronomy/src/observer.rs`
- `crates/renderer/src/camera.rs`

## Generated / embedded data

Some data is transformed at build time or embedded for WASM / single-binary use.

Current generated / embedded paths:

- `crates/catalog/build.rs` for embedded catalog support;
- `crates/renderer/build.rs` for compact constellation data and generated
  label metadata (`label_data.rs` in Cargo `OUT_DIR`);
- `docs/assets/readme/*.png`, generated by
  `scripts/generate-readme-images.sh` from deterministic CLI scenes using the
  local HYG catalog and renderer defaults.

Rules:

1. Generated output should be deterministic.
2. The source file and transform should be documented.
3. If generated data is checked in, explain why.
4. If generated data changes rendering or numerical output, add or update tests.

## Future data sources to document

When these roadmap items are implemented, add details here:

- Hipparcos catalog;
- Tycho-2 catalog;
- Gaia DR3 catalog;
- SIMBAD / VizieR identifier linking;
- DE440 ephemeris data;
- Messier catalog;
- NGC / IC catalog;
- AAVSO variable-star light curves;
- telescope / eyepiece preset data;
- deterministic scene presets and validation-gallery session files;
- curated public demo-gallery session files;
- large-catalog spatial indexes, LOD subsets, or WASM-specific extracts;
- notebook example input/output fixtures.

For each future source, record:

- exact source URL or archive identifier;
- version / release date;
- license and redistribution terms;
- local path;
- preprocessing command;
- fields used;
- known limitations.
