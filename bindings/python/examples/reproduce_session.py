#!/usr/bin/env python3
"""Binding-native session reproducibility example (L-21).

The original ``examples/notebooks/session_reproducibility.py`` reproduces the
renderer's numbers by shelling out to the CLI and parsing its CSV/JSON. This
example does the same cross-check *in-process* through the ``stars_py`` PyO3
binding: it loads a committed scene-preset session, rebuilds the renderer's
``Observer`` from it, and prints the apparent Sun/Moon/planet geometry plus the
observation-planning and occultation/eclipse surfaces — all from the exact same
``astronomy`` functions the renderer consumes, with no subprocess.

Build the binding first, then run:

    cd bindings/python && maturin develop --features extension-module
    python examples/reproduce_session.py

This is an example, not a CI gate: ``make notebook-check`` still drives the
stdlib-only notebook so it keeps working without a Python extension build.
"""

from __future__ import annotations

import math
import sys
from pathlib import Path

try:
    import stars_py
except ImportError as exc:  # pragma: no cover - dev-time hint
    sys.stderr.write(
        "stars_py is not importable. Build it first:\n"
        "  cd bindings/python && maturin develop --features extension-module\n"
        f"original error: {exc}\n"
    )
    raise SystemExit(1) from exc

ROOT = Path(__file__).resolve().parents[3]
SESSION = ROOT / "docs" / "presets" / "sessions" / "tokyo-tonight.json"


def main() -> None:
    # 1. Load a committed renderer session and rebuild its Observer.
    session = stars_py.Session.from_json(SESSION.read_text())
    obs = session.observer()
    print(f"Loaded {SESSION.name} (schema v{session.schema_version})")
    print(f"  observer: lat={obs.latitude_deg:.4f}° lon={obs.longitude_deg:.4f}°")
    print(f"  jd_utc={obs.jd_utc:.6f} jd_tt={obs.jd_tt:.6f}")

    # 2. Apparent geometry — identical numerics to what the renderer draws.
    sun_moon = stars_py.apparent_sun_moon(obs)
    sun_alt, _ = sun_moon.sun.altaz(obs)
    moon_alt, _ = sun_moon.moon.altaz(obs)
    print(f"  Sun alt={math.degrees(sun_alt):.3f}°  Moon alt={math.degrees(moon_alt):.3f}°")

    # 3. Planning surface from the loaded session.
    plan = stars_py.evening_plan(obs)
    print(f"  evening window: {plan.start_jd_utc:.4f} → {plan.end_jd_utc:.4f}")

    # 4. Occultation / eclipse search across the evening window.
    jd0, jd1 = plan.start_jd_utc, plan.end_jd_utc
    eclipse = stars_py.find_solar_eclipse(obs, jd0, jd1)
    print(f"  solar eclipse this window: {eclipse!r}")
    for occ in stars_py.active_occluders(obs):
        print(f"  active occluder: {occ.target} ({occ.kind}, {occ.obscuration:.3f})")

    print("OK: reproduced renderer numerics in-process via stars_py")


if __name__ == "__main__":
    main()
