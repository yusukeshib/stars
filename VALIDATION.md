# Validation

`stars` aims to be visually useful while keeping the scientific choices behind
its output explicit. This document records the validation policy: what should be
pinned by tests, what external references matter, and where the current limits
are.

The roadmap-level goal is not that every subsystem is publication-grade today.
The goal is that each approximation is named, tested, and easy to replace when
a higher-precision model lands.

## Validation principles

1. **Name the model.** If code implements a standard, paper, or approximation,
   the code or docs should say which one.
2. **Pin representative values.** Numerical behaviour should have tests that
   fail on silent drift.
3. **Separate visual approximations from astronomical claims.** A rendering aid
   can be approximate, but it must not be documented as high-precision science.
4. **Prefer tolerances with meaning.** Use angular tolerances for positions,
   magnitude / luminance tolerances for photometry, and explicit time tolerances
   for planning events.
5. **Document model domains.** A daylight model, twilight model, refraction
   approximation, or ephemeris approximation should state where it is expected
   to be valid.

## What must be tested

Add or update tests when changing any of these areas:

- leap-second handling;
- UTC / UT1 / TAI / TT / TDB conversion;
- sidereal time;
- precession, nutation, annual aberration, proper motion, refraction;
- equatorial / horizontal conversion;
- Sun, Moon, or planet apparent positions;
- topocentric parallax;
- moon phase or eclipse-darkening support;
- rise / transit / set;
- twilight band boundaries;
- magnitude, illuminance, luminance, extinction, skyglow, daylight, twilight,
  and tone reproduction reference models;
- catalog coordinate conversion and colour conversion.

## Current validation coverage by subsystem

### Time systems

Current implementation:

- UTC civil input;
- UT1 for Earth rotation;
- TAI and TT from a built-in leap-second table;
- approximate TDB for ephemeris use;
- DUT1 defaults to zero unless supplied.

Validation expectation:

- J2000 and known leap-second boundaries should remain pinned.
- UT1-sensitive results should document whether DUT1 is supplied or assumed
  zero.

Current limitation:

- DUT1 is not automatically fetched from IERS bulletins.

### Coordinate transforms and stellar corrections

Current implementation includes:

- J2000 / ICRS-like Cartesian catalog positions;
- proper-motion propagation;
- IAU 2006 precession;
- compact IAU-2000-style nutation;
- first-order annual aberration;
- Saemundsson-style atmospheric refraction.

Validation expectation:

- Matrix transforms should preserve vector length within floating-point
  tolerance.
- Known epochs and simple geometric cases should be pinned.
- Refraction should be tested at representative altitudes and disabled / low
  pressure cases.

Current limitation:

- The nutation model is compact rather than a full SOFA-equivalent series.
- Refraction is an empirical visual correction, not a full ray-tracing model.

### Solar-system positions

Current implementation includes visually useful Sun, Moon, and planet apparent /
topocentric inputs.

Validation expectation:

- Representative dates and observer locations should have pinned apparent
  directions.
- Topocentric correction should be tested separately from geocentric direction
  where possible.
- Angular radius and phase values should be pinned for simple dates.

Current limitation:

- Phase 3 still tracks higher-precision DE440 / publication-grade ephemeris
  work. Do not describe the current stack as final research-grade ephemerides.

### Atmosphere and sky colour

Current implementation includes:

- stellar extinction;
- diffuse night-sky glow;
- zodiacal light, gegenschein, airglow, and dust extinction;
- solar and lunar illuminants;
- daylight scattering approximation;
- twilight radiance model;
- moonlit sky composition.

Validation expectation:

- Model-domain boundaries should be pinned: noon, sunset, civil twilight,
  nautical twilight, astronomical twilight, and moonlit night.
- Changes to coefficients should explain their source.
- Discontinuities across day / twilight / night transitions should be tested or
  visually justified.

Current limitation:

- The atmosphere is a compact renderer-oriented model, not a full spectral
  radiative-transfer simulation.
- Weather, clouds, local light pollution, and terrain obstruction are not
  modeled.

### Photometry and tone reproduction

Current implementation includes:

- magnitude-to-illuminance mapping;
- mesopic and scotopic adaptation behaviour;
- HDR accumulation;
- glare / PSF approximation;
- adaptive tonemapping.

Validation expectation:

- Monotonicity and representative value tests should protect brightness and
  colour response.
- Any display-facing heuristic should be documented as a rendering choice rather
  than an astronomical measurement.

Current limitation:

- Final output depends on display characteristics and is not colour-managed for
  every monitor.

### Planning helpers

Current implementation includes:

- evening window selection;
- rise / transit / set;
- twilight indicators;
- web planning table.

Validation expectation:

- Edge cases should be covered: always above horizon, always below horizon,
  high latitude, date boundaries, and twilight bands that do not occur.

Current limitation:

- Terrain horizon and weather constraints are not modeled.

## External comparison targets

Use these as comparison targets when practical:

- IAU / SOFA algorithms for time and coordinate standards;
- JPL Horizons for apparent solar-system positions;
- published photometry and atmosphere references named in `ROADMAP.md`;
- known catalog source documentation for star data and identifiers.

When adding a comparison, record:

- external tool / dataset version;
- input time scale;
- observer location;
- target body or star;
- output coordinate frame;
- tolerance;
- date when the reference value was captured.

## Testing style

Prefer small tests that pin a specific contract:

```rust
#[test]
fn j2000_utc_maps_to_expected_julian_date() {
    let ts = TimeScales::from_unix_seconds(946728000.0);
    assert!((ts.jd_utc - 2_451_545.0).abs() < 1e-9);
}
```

For approximate models, avoid over-tight tolerances that imply false precision.
The test should catch regressions while respecting the intended model accuracy.

## Documentation requirements for new scientific models

A PR adding or replacing a scientific model should update docs with:

- model name;
- citation or source;
- valid domain;
- expected error or approximation level;
- tests added;
- affected hosts;
- whether the model is visual-only or intended for numerical astronomy.
