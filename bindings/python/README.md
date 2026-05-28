# stars-py — Python bindings for `stars` astronomy + catalog (L-21)

PyO3 wrapper around the read-only `astronomy` + `catalog` public surface
that the rest of the project (`apps/cli`, `apps/viewer`, `apps/web`)
already consumes. The goal is reproducibility-by-binding: a notebook
reviewer can call the exact same `apparent_sun_moon` /
`apparent_planets` / `apparent_galilean_moons` functions the renderer
calls, with bit-identical numerics.

Side-effects beyond loading the embedded star catalog are out of scope
for this rung — no rendering, no scene JSON parsing, no file I/O.

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

| Python symbol | Wraps |
|---|---|
| `Observer(lat_deg, lon_deg, jd_utc)` | `astronomy::Observer::from_degrees` |
| `Observer.from_unix_seconds(...)` | `julian_date_from_unix_seconds` + ctor |
| `observer_from_unix_seconds(...)` | same, module-level convenience |
| `apparent_sun_moon(observer)` | `SunMoonApparent::for_observer` |
| `apparent_planets(observer)` | `apparent_planets_topocentric` |
| `apparent_galilean_moons(observer)` | `apparent_galilean_moons_topocentric` |
| `apparent_titan(observer)` | `apparent_titan_topocentric` |
| `StarCatalog.load_embedded()` | `catalog::load_embedded` |

Returned objects expose the renderer-facing fields (`right_ascension_rad`,
`declination_rad`, `magnitude`, `position`, `color`, …) plus an
`.altaz(observer)` method that runs `equatorial_to_horizontal` with the
observer's LST and latitude. Catalog identifiers (`hyg_id`, `hip_id`)
are surfaced as `Optional[int]` so a notebook can cross-reference HYG
or Hipparcos lookups directly.

## Scope and non-goals

- **Read-only.** No `Session` writes, no scene rendering, no GPU.
- **No wheels in CI yet.** The wheel-build matrix is tracked under the
  L-21 follow-up scope; `make pyo3-check` is the present gate.
- **Identifier preservation** through the renderer is still under
  `L-18`; `Star.hyg_id` reflects what the embedded backend records,
  which is currently `None` for the compact catalog rows.
