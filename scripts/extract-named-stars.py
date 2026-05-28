#!/usr/bin/env python3
"""Regenerate `crates/catalog/data/named_stars.tsv` from HYG v4.2.

The output is the V-56 object-search index. The filter keeps every HYG row
that meets one of the following thresholds:

    proper name present       and V <= 6.0
    Bayer designation present and V <= 5.0
    Flamsteed number present  and V <= 4.5

The synthetic Sun row (`proper == "Sol"`) is dropped: the renderer uses the
ephemeris path for the Sun, and including it here would let users type
"Sol" and slew the camera to a stellar catalog row with `mag = -26.7`.

Run from the repository root:

    python3 scripts/extract-named-stars.py
"""
from __future__ import annotations

import csv
import pathlib

INPUT = pathlib.Path("crates/catalog/data/hyg_v42.csv")
OUTPUT = pathlib.Path("crates/catalog/data/named_stars.tsv")


def parse_float(value: str) -> float:
    try:
        return float(value)
    except ValueError:
        return float("nan")


def main() -> None:
    rows: list[tuple[str, ...]] = []
    with INPUT.open() as f:
        reader = csv.DictReader(f)
        for row in reader:
            try:
                mag = float(row["mag"])
            except ValueError:
                continue
            proper = row["proper"].strip()
            bayer = row["bayer"].strip()
            flam = row["flam"].strip()
            if proper == "Sol":
                continue
            keep = (
                (bool(proper) and mag <= 6.0)
                or (bool(bayer) and mag <= 5.0)
                or (bool(flam) and mag <= 4.5)
            )
            if not keep:
                continue
            ra = parse_float(row["rarad"])
            dec = parse_float(row["decrad"])
            if not (ra == ra and dec == dec):  # noqa: PLR0124 -- NaN check
                continue
            dist = parse_float(row.get("dist", ""))
            if dist != dist or dist <= 0 or dist >= 100000:
                dist = 0.0
            rows.append(
                (
                    proper,
                    bayer,
                    flam,
                    row["hr"].strip(),
                    row["hd"].strip(),
                    row["hip"].strip(),
                    row["con"].strip(),
                    f"{ra:.7f}",
                    f"{dec:.7f}",
                    f"{mag:.3f}",
                    f"{dist:.3f}",
                )
            )

    rows.sort(key=lambda r: float(r[9]))
    with OUTPUT.open("w") as f:
        f.write("# proper\tbayer\tflam\thr\thd\thip\tcon\tra_rad\tdec_rad\tmag\tdist_pc\n")
        for r in rows:
            f.write("\t".join(r) + "\n")
    print(f"wrote {len(rows)} entries to {OUTPUT}")


if __name__ == "__main__":
    main()
