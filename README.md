# stars

日本語版: [`README.ja.md`](README.ja.md)

`stars` is a physically informed, cross-platform sky renderer written in Rust.
It targets two uses at once:

- a casual night-sky viewer that helps people understand what they are seeing;
- a defensible astronomy / education engine whose coordinate transforms,
  time systems, photometry, and atmosphere choices are explicit and tested.

The project currently ships three reference hosts over a shared Rust engine:

- `apps/cli` — headless PNG rendering;
- `apps/viewer` — interactive native desktop viewer;
- `apps/web` — WASM + browser UI.

## What works today

The current engine already includes:

- horizon, cardinal, alt-az, equatorial, ecliptic, meridian, galactic, and
  constellation overlays;
- constellation lines, IAU/Delporte constellation boundaries, and text labels
  for bright stars, Sun/Moon/planets, constellations, cardinal directions, and degrees;
- physical star brightness / colour pipeline with HDR rendering, atmospheric
  extinction, glare, mesopic/scotopic adaptation, diffuse sky glow, zodiacal
  light, airglow, and dust extinction;
- UTC / UT1 / TAI / TT / approximate TDB time scales;
- proper motion, annual aberration, IAU 2006 precession, compact nutation, and
  atmospheric refraction;
- Sun, Moon, Mercury through Neptune, moon phase / eclipse darkening, daylight
  and twilight sky colour;
- web planning helpers for rise / transit / set and twilight intervals;
- perspective plus Mollweide, Aitoff, and Hammer full-sky projections;
- shareable web session URLs.

See [`PROGRESS.md`](PROGRESS.md) for the implementation log and
[`ROADMAP.md`](ROADMAP.md) for remaining work.

## Quick start

```bash
make setup
make viewer
```

Render a PNG from the CLI:

```bash
make cli ARGS="--lat 35.68 --lng 139.69 --azimuth 180 --altitude 30 -o stars.png"
```

Run the web app:

```bash
make web
```

Run the full local CI suite:

```bash
make ci
```

## Repository layout

```txt
crates/astronomy   astronomical time, coordinates, corrections, ephemerides,
                   photometry, atmosphere, skyglow, planning helpers
crates/catalog     HYG catalog loading and colour / coordinate conversion
crates/renderer    wgpu renderer, camera, overlays, tonemap, star instances
crates/common      native-host glue shared by CLI and desktop viewer
apps/cli           headless PNG renderer
apps/viewer        native winit desktop viewer
apps/web           WASM engine wrapper and frontend UI
scripts            catalog download and WASM build helpers
```

More detail lives in [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Documentation

- [`ROADMAP.md`](ROADMAP.md) — phase plan, open work, and exit criteria.
- [`PROGRESS.md`](PROGRESS.md) — completed implementation log.
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — crate boundaries, data flow,
  coordinate conventions, renderer pipeline, and host integration.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — setup, checks, PR expectations, and
  numerical-change rules.
- [`VALIDATION.md`](VALIDATION.md) — scientific / numerical validation policy
  and current model limits.
- [`DATA_SOURCES.md`](DATA_SOURCES.md) — catalog, constellation, and literature
  data provenance.

## Current focus

Phase 1, Phase 1', Phase 2, and Phase 4 full-sky projection work are
implemented. The most useful remaining visual work is now Phase 4 rendering
polish, in this order:

1. out-of-Earth / galactic viewpoint experiments;
2. deep-sky overlays for Messier / NGC-style objects;
3. telescope eyepiece simulation.

## Contributing

Before changing code, read [`CONTRIBUTING.md`](CONTRIBUTING.md). Any change that
affects numerical astronomy output must add or update a pinned test so silent
scientific drift is caught in CI.
