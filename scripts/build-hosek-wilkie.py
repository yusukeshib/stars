#!/usr/bin/env python3
"""Regenerate the embedded Hošek-Wilkie 2012 RGB sky-dome coefficient table.

Downloads the upstream sample-code archive from the Czech CGG release page,
parses ``ArHosekSkyModelData_RGB.h``, and writes a packed little-endian binary
to ``crates/astronomy/data/hosek_wilkie/coefficients_rgb.bin``. The committed
binary is hashed in ``data/manifest.toml``; ``make manifest-check`` will fail
until the hash is re-pinned after regeneration.

Binary layout (little-endian, ``f64`` = IEEE 754 binary64):

    bytes 0..8       ASCII magic ``HW2012RG``
    bytes 8..12      u32 version (currently 1)
    bytes 12..16     u32 reserved (0)
    bytes 16...      coefficients[3][2][10][6][9] f64
                       channel × albedo × turbidity × elev-control × A..I
    next             radiances[3][2][10][6] f64
                       channel × albedo × turbidity × elev-control

Channel order: R, G, B (matches ``datasetRGB1/2/3`` in the upstream archive).
Albedo entries: 0.0, then 1.0 (the upstream cooker linearly blends in albedo).
Turbidities 1..10 stored consecutively; elev-control 0..5 are the quintic
Bezier control points the cooker mixes via ``(elev / (π/2))^(1/3)``.

References
----------
* Hošek, L., Wilkie, A. 2012, ACM TOG 31(4), "An Analytic Model for Full
  Spectral Sky-Dome Radiance".
* Upstream sample-code release v1.4a, 22 Feb 2013 (BSD 3-clause):
  https://cgg.mff.cuni.cz/projects/SkylightModelling/
"""

from __future__ import annotations

import argparse
import io
import re
import struct
import sys
import urllib.request
import zipfile
from pathlib import Path

UPSTREAM_URL = (
    "https://cgg.mff.cuni.cz/projects/SkylightModelling/"
    "HosekWilkie_SkylightModel_C_Source.1.4a.zip"
)
UPSTREAM_VERSION = "1.4a"
HEADER_NAME = "ArHosekSkyModelData_RGB.h"

# Cooker contract (see ArHosekSkyModel.c::ArHosekSkyModel_CookConfiguration):
#   datasetRGBn layout per channel:
#     [albedo 0 turbidity 1..10][albedo 1 turbidity 1..10]
#   each turbidity block: 6 elev-control points × 9 coefficients
#   → 2 * 10 * 6 * 9 = 1080 doubles per channel
N_CHANNELS = 3
N_ALBEDOS = 2
N_TURBIDITIES = 10
N_ELEV_CONTROL = 6
N_COEFFS = 9
COEFFS_PER_CHANNEL = N_ALBEDOS * N_TURBIDITIES * N_ELEV_CONTROL * N_COEFFS  # 1080

# datasetRGBRadn layout per channel: same albedo/turbidity structure, but each
# block is just 6 doubles (one radiance scalar per elev-control point).
#   → 2 * 10 * 6 = 120 doubles per channel
RADIANCES_PER_CHANNEL = N_ALBEDOS * N_TURBIDITIES * N_ELEV_CONTROL  # 120

MAGIC = b"HW2012RG"
VERSION = 1

OUT_PATH = Path("crates/astronomy/data/hosek_wilkie/coefficients_rgb.bin")


def fetch_upstream_header() -> str:
    """Download the upstream zip and return ArHosekSkyModelData_RGB.h as text."""
    print(f"fetching {UPSTREAM_URL}", file=sys.stderr)
    with urllib.request.urlopen(UPSTREAM_URL) as resp:
        blob = resp.read()
    with zipfile.ZipFile(io.BytesIO(blob)) as zf:
        # The upstream archive nests files inside a versioned directory.
        candidates = [n for n in zf.namelist() if n.endswith("/" + HEADER_NAME)]
        if not candidates:
            raise SystemExit(f"{HEADER_NAME} not found in upstream archive")
        return zf.read(candidates[0]).decode("utf-8")


def parse_double_arrays(source: str) -> dict[str, list[float]]:
    """Extract ``datasetRGB{1,2,3}`` and ``datasetRGBRad{1,2,3}`` arrays."""
    arrays: dict[str, list[float]] = {}
    # Match: double NAME[] = { ... }; with possibly nested braces (none here).
    pattern = re.compile(
        r"double\s+(datasetRGB(?:Rad)?[123])\s*\[\s*\]\s*=\s*\{([^}]*)\}\s*;",
        re.DOTALL,
    )
    for match in pattern.finditer(source):
        name = match.group(1)
        body = match.group(2)
        # Strip C/C++ line comments (the upstream interleaves "// albedo X turb Y"
        # tags between value rows) and split on commas.
        stripped = re.sub(r"//[^\n]*", "", body)
        tokens = [t.strip() for t in stripped.split(",") if t.strip()]
        arrays[name] = [float(t) for t in tokens]
    return arrays


def write_packed(out_path: Path, arrays: dict[str, list[float]]) -> None:
    """Pack the six arrays into the documented binary layout."""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("wb") as out:
        out.write(MAGIC)
        out.write(struct.pack("<II", VERSION, 0))
        # Coefficients: channel × albedo × turbidity × elev × 9
        for channel in (1, 2, 3):
            name = f"datasetRGB{channel}"
            values = arrays[name]
            if len(values) != COEFFS_PER_CHANNEL:
                raise SystemExit(
                    f"{name}: expected {COEFFS_PER_CHANNEL} doubles, "
                    f"got {len(values)}"
                )
            out.write(struct.pack(f"<{len(values)}d", *values))
        # Radiances: channel × albedo × turbidity × elev
        for channel in (1, 2, 3):
            name = f"datasetRGBRad{channel}"
            values = arrays[name]
            if len(values) != RADIANCES_PER_CHANNEL:
                raise SystemExit(
                    f"{name}: expected {RADIANCES_PER_CHANNEL} doubles, "
                    f"got {len(values)}"
                )
            out.write(struct.pack(f"<{len(values)}d", *values))


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument(
        "--source",
        type=Path,
        default=None,
        help=(
            f"Path to a local copy of {HEADER_NAME}. When omitted the "
            f"upstream archive is fetched from {UPSTREAM_URL}."
        ),
    )
    ap.add_argument(
        "--out",
        type=Path,
        default=OUT_PATH,
        help=f"Output binary path (default: {OUT_PATH}).",
    )
    args = ap.parse_args()

    if args.source is not None:
        print(f"reading {args.source}", file=sys.stderr)
        source = args.source.read_text(encoding="utf-8")
    else:
        source = fetch_upstream_header()

    arrays = parse_double_arrays(source)
    expected = {f"datasetRGB{i}" for i in (1, 2, 3)} | {
        f"datasetRGBRad{i}" for i in (1, 2, 3)
    }
    missing = expected - arrays.keys()
    if missing:
        raise SystemExit(f"missing arrays in source: {sorted(missing)}")

    write_packed(args.out, arrays)
    print(
        f"wrote {args.out} ({args.out.stat().st_size} bytes; "
        f"upstream version {UPSTREAM_VERSION})",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
