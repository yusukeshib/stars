# stars-py — Python bindings for `stars` astronomy + catalog (L-21)

PyO3 wrapper around the `astronomy` + `catalog` public surface that the
rest of the project (`apps/cli`, `apps/viewer`, `apps/web`) already
consumes. The goal is reproducibility-by-binding: a notebook reviewer
can call the exact same apparent-body, observation-planning, and
session functions the renderer uses, with bit-identical numerics.

The binding stays off the renderer / WGPU / CLI dependency path. The
session round-trip rides on `serde_json` alone (not the host
`stars-host-common` crate, which pulls in `clap` / `chrono` / `wgpu`),
and the only side-effects are loading the embedded catalog and the
optional `Session.load` / `.save` file helpers. No rendering, no GPU.

## Build

The binding is built as a Python extension module via
[maturin](https://www.maturin.rs/). A Python ≥3.9 toolchain is required
at build time, and PyO3 emits an `abi3-py39` ABI-stable wheel so the
same binary works on every newer interpreter.

```bash
cd bindings/python
python -m venv .venv
source .venv/bin/activate
pip install maturin
maturin develop --features extension-module
python tests/smoke.py
```

`make ci` does **not** build a wheel; it runs `cargo check -p stars-py`
(via `make pyo3-check`) and the in-crate Rust unit tests, which exercise
the same wrapper types through a pure-Rust entry point. The
`extension-module` feature is opt-in so plain `cargo check` does not
need to find a Python interpreter.

## API surface

### Bodies and catalog

| Python symbol | Wraps |
|---|---|
| `Observer(lat_deg, lon_deg, jd_utc)` | `astronomy::Observer::from_degrees` |
| `Observer.from_unix_seconds(...)` | `julian_date_from_unix_seconds` + ctor |
| `observer_from_unix_seconds(...)` | same, module-level convenience |
| `julian_date_from_unix_seconds(s)` | `astronomy::julian_date_from_unix_seconds` |
| `jd_utc_to_unix_ms(jd)` | `astronomy::jd_utc_to_unix_ms` |
| `apparent_sun_moon(observer)` | `SunMoonApparent::for_observer` |
| `apparent_planets(observer)` | `apparent_planets_topocentric` |
| `apparent_galilean_moons(observer)` | `apparent_galilean_moons_topocentric` |
| `apparent_titan(observer)` | `apparent_titan_topocentric` |
| `StarCatalog.load_embedded()` | `catalog::load_embedded` |

Returned body objects expose the renderer-facing fields
(`right_ascension_rad`, `declination_rad`, `magnitude`, `position`,
`color`, …) plus an `.altaz(observer)` method that runs
`equatorial_to_horizontal` with the observer's LST and latitude. Catalog
identifiers (`hyg_id`, `hip_id`) are surfaced as `Optional[int]`.

### Observation planning

| Python symbol | Wraps |
|---|---|
| `evening_plan(observer)` → `EveningPlan` | `astronomy::evening_plan` |
| `rise_transit_set(observer, body, start_jd, end_jd)` | `astronomy::rise_transit_set` |
| `twilight_indicators(observer, start_jd, end_jd)` | `astronomy::twilight_indicators` |
| `twilight_band(sun_alt_rad)` → `str` | `astronomy::twilight_band` |
| `body_altitude_rad(observer, body)` | `astronomy::body_altitude_rad` |

`body` is a case-insensitive name: `"sun"`, `"moon"`, or `"mercury"` …
`"neptune"`. `EveningPlan` carries `.rows` (a `RiseTransitSet` per
default planning body) and `.twilight` (the ordered band timeline);
`RiseTransitSet` times are `Optional[float]` UTC Julian Dates.

### Session round-trip (`crates/common` JSON schema)

| Python symbol | Behaviour |
|---|---|
| `Session(lat, lon, jd_utc, azimuth_deg=0, altitude_deg=45, fov_deg=85)` | build from the current-schema template |
| `Session.from_observer(observer, azimuth_deg=…, …)` | build from an `Observer`, preserving its time scales |
| `Session.from_json(text)` / `Session.load(path)` | parse, preserving unknown fields |
| `session.to_json(pretty=True)` / `session.save(path)` | serialise |
| `session.observer()` → `Observer` | bridge a loaded session into the query API |

`Session` exposes typed `latitude_deg` / `longitude_deg` / `jd_utc` /
`jd_ut1` / `jd_tt` / `jd_tdb` / `azimuth_deg` / `altitude_deg` /
`fov_deg` / `schema_version` properties. Setting `jd_utc` recomputes the
dependent time scales with the same `astronomy::TimeScales` helper the
renderer uses, and every field the binding does not edit (overlays,
atmosphere, projection, eyepiece, corrections) is preserved verbatim on
round-trip.

## Scope and non-goals

- **No schema duplication.** `Session` wraps the parsed JSON value and
  seeds new documents from the committed `dark-sky` preset, so it tracks
  the real `SESSION_SCHEMA_VERSION` without re-declaring fields.
- **No wheels in CI yet.** The wheel-build matrix is tracked under the
  L-21 follow-up scope; `make pyo3-check` is the present gate.
- **Identifier preservation** through the renderer is still under
  `L-18`; `Star.hyg_id` reflects what the embedded backend records,
  which is currently `None` for the compact catalog rows.
