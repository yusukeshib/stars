#!/usr/bin/env python3
"""Regenerate `crates/catalog/data/bright_star_xmatch.csv` from HYG v4.2.

The output is the L-17 bright-star HIP <-> HD cross-identification anchor index.
It is derived deterministically from the in-repo HYG catalogue (which already
carries `hip` and `hd` columns), so it needs no network access and is fully
reproducible. The Tycho-2 and Gaia DR3 cross-IDs are intentionally NOT included
here: those are filled at ingest time from the catalogue backends / a VizieR
cross-match, not hand-entered (see crates/catalog/src/ingest.rs).

Selection: every HYG row that has BOTH a Hipparcos and an HD number and is
brighter than V = 3.0 (the naked-eye anchor set), excluding the synthetic Sun
row (`proper == "Sol"`).

Run from the repository root:

    python3 scripts/extract-bright-star-xmatch.py
"""
from __future__ import annotations

import csv
import pathlib

INPUT = pathlib.Path("crates/catalog/data/hyg_v42.csv")
OUTPUT = pathlib.Path("crates/catalog/data/bright_star_xmatch.csv")
MAG_LIMIT = 3.0


def main() -> None:
    rows: list[tuple[int, int, float, float, str]] = []
    with INPUT.open() as f:
        reader = csv.DictReader(f)
        for row in reader:
            hip = row.get("hip", "").strip()
            hd = row.get("hd", "").strip()
            proper = row.get("proper", "").strip()
            if not hip or not hd or proper == "Sol":
                continue
            try:
                mag = float(row["mag"])
            except ValueError:
                continue
            if mag > MAG_LIMIT:
                continue
            try:
                hip_n = int(hip)
                hd_n = int(hd)
            except ValueError:
                continue
            try:
                bv = float(row.get("ci", "") or "nan")
            except ValueError:
                bv = float("nan")
            rows.append((hip_n, hd_n, mag, bv, proper))

    # Deterministic ordering by HIP keeps the committed file diffable.
    rows.sort(key=lambda r: r[0])

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with OUTPUT.open("w", newline="") as f:
        writer = csv.writer(f, lineterminator="\n")
        writer.writerow(["hip", "hd", "vmag", "bv", "proper"])
        for hip_n, hd_n, mag, bv, proper in rows:
            bv_out = "" if bv != bv else f"{bv:.3f}"  # NaN check
            writer.writerow([hip_n, hd_n, f"{mag:.2f}", bv_out, proper])

    print(f"wrote {len(rows)} bright-star anchors to {OUTPUT}")


if __name__ == "__main__":
    main()
