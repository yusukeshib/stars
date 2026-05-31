#!/usr/bin/env python3
"""Resample the Falchi et al. 2016 World Atlas GeoTIFF into the compact
``FALATL01`` grid the stars V-39-Atlas light-pollution loader consumes.

The published raster (``World_Atlas_2015.tif``) stores the **ratio of
artificial zenith sky brightness to the natural background**, where Falchi
et al. adopt a natural reference of 0.174 mcd/m^2 (V ~= 21.6 mag/arcsec^2).
This script:

1. reads that ratio raster (band 1) with rasterio,
2. block-averages it down to a coarse regular lat/lng grid (default 0.1 deg),
3. converts each cell's mean ratio ``r`` to a **total** V-band zenith surface
   brightness via the flux-additive model

       mu_total = MU_NATURAL - 2.5 * log10(1 + r)

   with ``MU_NATURAL = 21.6`` mag/arcsec^2 -- the same natural floor the
   renderer's dark-sky composition uses (``skyglow::NATURAL_FLOOR_S10`` ->
   V ~= 21.6), so atlas-sampled and Bortle/SQM paths stay on one scale, and
4. writes the little-endian ``FALATL01`` binary documented in
   ``crates/astronomy/src/light_pollution_atlas.rs``.

No-data / negative-ratio cells (ocean, outside the VIIRS swath) are written as
NaN; the Rust sampler skips them and falls back to the natural floor.

Usage:
    scripts/build-falchi-atlas.py INPUT.tif OUTPUT.bin [--step-deg 0.1]

Provenance for the output grid is recorded in data/manifest.toml /
DATA_SOURCES.md against the upstream GeoTIFF (DOI 10.5880/GFZ.1.4.2016.001).
"""
from __future__ import annotations

import argparse
import struct
import sys

MAGIC = b"FALATL01"
MU_NATURAL = 21.6  # V mag/arcsec^2 natural floor (matches the renderer)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("input_tif", help="World Atlas 2015 GeoTIFF (artificial/natural ratio)")
    ap.add_argument("output_bin", help="destination FALATL01 grid")
    ap.add_argument(
        "--step-deg",
        type=float,
        default=0.1,
        help="output cell size in degrees (default 0.1)",
    )
    args = ap.parse_args()

    try:
        import numpy as np
        import rasterio
        from rasterio.enums import Resampling
    except ImportError as exc:  # pragma: no cover - tooling guard
        print(
            f"error: this script needs numpy + rasterio ({exc}).\n"
            "  pip install numpy rasterio",
            file=sys.stderr,
        )
        return 1

    with rasterio.open(args.input_tif) as src:
        bounds = src.bounds  # left, bottom, right, top in degrees (EPSG:4326)
        out_w = max(1, int(round((bounds.right - bounds.left) / args.step_deg)))
        out_h = max(1, int(round((bounds.top - bounds.bottom) / args.step_deg)))
        # Average (block-mean) downsample of the ratio band.
        ratio = src.read(
            1,
            out_shape=(out_h, out_w),
            resampling=Resampling.average,
        ).astype("float64")
        nodata = src.nodata

    # Mark no-data and physically-impossible negative ratios as NaN.
    if nodata is not None:
        ratio = np.where(ratio == nodata, np.nan, ratio)
    ratio = np.where(ratio < 0.0, np.nan, ratio)

    # Flux-additive conversion to total V mag/arcsec^2 (north-up, row 0 = top).
    with np.errstate(invalid="ignore"):
        mu = MU_NATURAL - 2.5 * np.log10(1.0 + ratio)
    mu = mu.astype("<f4")  # little-endian f32, row-major

    header = MAGIC + struct.pack(
        "<IIdddd",
        out_h,                 # rows (latitude, north -> south)
        out_w,                 # cols (longitude, west -> east)
        float(bounds.top),     # lat_north_deg
        float(bounds.bottom),  # lat_south_deg
        float(bounds.left),    # lng_west_deg
        float(bounds.right),   # lng_east_deg
    )
    with open(args.output_bin, "wb") as fh:
        fh.write(header)
        fh.write(mu.tobytes(order="C"))

    print(
        f"wrote {args.output_bin}: {out_h}x{out_w} cells, "
        f"bounds lat[{bounds.bottom:.3f},{bounds.top:.3f}] "
        f"lng[{bounds.left:.3f},{bounds.right:.3f}]",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
