#!/usr/bin/env python3
"""Smoke test for the `stars_py` PyO3 binding (L-21).

Builds an Observer for Tokyo at 2026-06-21T10:20:00Z — the same epoch the
``civil-twilight-antisolar-tokyo`` scene preset renders against — and
prints the Moon's apparent altitude alongside the Sun's, plus the first
few planets and the embedded star catalog's size. The numerics must match
what ``apps/cli`` renders for the same session JSON; that property is
what makes the binding useful for notebook-side reproduction.

Run after ``maturin develop`` from this directory.
"""

from __future__ import annotations

import math
import sys

try:
    import stars_py
except ImportError as exc:  # pragma: no cover - dev-time hint
    sys.stderr.write(
        "stars_py is not importable. Build the wheel first:\n"
        "  cd bindings/python && maturin develop --features extension-module\n"
        f"original error: {exc}\n"
    )
    raise SystemExit(1) from exc


# 2026-06-21T10:20:00Z, the V-27 civil-twilight-antisolar-tokyo epoch.
UNIX_2026_06_21_T_10_20_UTC = 1_782_555_600.0
TOKYO_LAT = 35.68
TOKYO_LON = 139.69


def main() -> None:
    obs = stars_py.observer_from_unix_seconds(
        TOKYO_LAT, TOKYO_LON, UNIX_2026_06_21_T_10_20_UTC
    )
    print(f"Observer: {obs!r}")
    print(f"  jd_utc = {obs.jd_utc:.6f}")
    print(f"  jd_tt  = {obs.jd_tt:.6f}")

    sun_moon = stars_py.apparent_sun_moon(obs)
    sun_alt, sun_az = sun_moon.sun.altaz(obs)
    moon_alt, moon_az = sun_moon.moon.altaz(obs)
    print(
        f"Sun:  alt = {math.degrees(sun_alt):7.3f}°, "
        f"az = {math.degrees(sun_az):7.3f}°"
    )
    print(
        f"Moon: alt = {math.degrees(moon_alt):7.3f}°, "
        f"az = {math.degrees(moon_az):7.3f}°"
    )

    planets = stars_py.apparent_planets(obs)
    for p in planets[:3]:
        alt, az = p.altaz(obs)
        print(
            f"  {p.name:<8s} alt={math.degrees(alt):7.3f}° "
            f"az={math.degrees(az):7.3f}° "
            f"mag={p.magnitude:5.2f}"
        )

    cat = stars_py.StarCatalog.load_embedded()
    print(f"Star catalog: {len(cat)} stars loaded")
    if len(cat) > 0:
        s = cat.star(0)
        print(
            f"  star[0] mag={s.magnitude:.2f} "
            f"color=({s.color[0]:.2f},{s.color[1]:.2f},{s.color[2]:.2f})"
        )


if __name__ == "__main__":
    main()
