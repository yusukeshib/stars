#!/usr/bin/env python3
"""Extract the bright-NGC/IC subset embedded by the renderer.

Downloads the upstream OpenNGC database (NGC.csv + addendum.csv) and writes
``crates/catalog/data/openngc_bright.csv``. The committed CSV is hashed in
``data/manifest.toml``; ``make manifest-check`` will fail until the hash is
re-pinned after regeneration.

Filter rules (kept identical to the build-script contract documented in the
committed CSV's header):

    * exclude Messier objects (already covered by ``messier.csv``);
    * exclude OpenNGC type codes ``*``, ``**``, ``*Ass``, ``NonEx`` and
      ``Other``;
    * keep ``min(V-Mag, B-Mag - 0.6)`` <= 11.5 mag when either band has a
      published value;
    * keep emission / reflection / diffuse nebulae (types ``EmN``, ``RfN``,
      ``Neb``, ``HII``, ``Cl+N``, ``SNR``) with major-axis >= 30 arcmin even
      when no integrated magnitude is published (sentinel mag = 99.00).

Output rows are sorted by ``(prefix, number, suffix)`` so the byte-for-byte
result is stable across machines.

References
----------
* Dreyer, J. L. E. 1888, MmRAS 49, 1 (NGC).
* Dreyer, J. L. E. 1908, MmRAS 59, 105 (IC).
* Verga, M. (current), OpenNGC repository — modernised NGC/IC compilation.
"""

from __future__ import annotations

import argparse
import csv
import re
import sys
import urllib.request
from pathlib import Path

UPSTREAM_NGC = "https://raw.githubusercontent.com/mattiaverga/OpenNGC/master/database_files/NGC.csv"
UPSTREAM_ADDENDUM = "https://raw.githubusercontent.com/mattiaverga/OpenNGC/master/database_files/addendum.csv"

MAG_CUTOFF = 11.5
LARGE_NEBULA_MIN_ARCMIN = 30.0
NO_PHOTOMETRY_SENTINEL = 99.0

# OpenNGC type -> normalised type used by the renderer's deepsky decoder.
KIND_MAP = {
    "OCl": "OC",
    "GCl": "GC",
    "G": "G",
    "PN": "PN",
    "SNR": "SNR",
    "EmN": "N",
    "RfN": "N",
    "Neb": "N",
    "HII": "N",
    "Cl+N": "OC",
    "GTrpl": "G",
    "GPair": "G",
    "GGroup": "G",
}
NEB_TYPES = {"EmN", "RfN", "Neb", "HII", "Cl+N", "SNR"}
HARD_SKIP_TYPES = {"*", "**", "*Ass", "NonEx", "Other"}

CSV_HEADER = """# Bright NGC / IC deep-sky object catalog (V <= 11.5 mag, plus large
# diffuse nebulae lacking integrated photometry).
#
# Source: OpenNGC (https://github.com/mattiaverga/OpenNGC) main NGC.csv
# and addendum.csv. Filter rules (see scripts/extract-openngc-bright.py):
#   - exclude Messier objects (covered by messier.csv);
#   - exclude OpenNGC types {*, **, *Ass, NonEx, Other};
#   - keep min(V-Mag, B-Mag - 0.6) <= 11.5 mag when photometry present;
#   - keep emission/reflection/diffuse nebulae (EmN/RfN/Neb/HII/Cl+N/SNR)
#     with major-axis >= 30 arcmin even when integrated magnitude is missing
#     (sentinel mag = 99.00; renderer treats them as faint until the
#     density slider is opened).
#
# Columns: name, ra_hours (J2000), dec_deg (J2000), mag, type, size_arcmin.
# Position units mirror crates/catalog/data/messier.csv. The renderer build
# script compacts this CSV into the i16 binary `openngc_bright.bin`.
#
# Type codes (normalised from OpenNGC):
#   OC  open cluster        GC  globular cluster
#   G   galaxy              N   diffuse / emission / reflection nebula
#   PN  planetary nebula    SNR supernova remnant
#   Other  anything else
"""


def parse_ra(value: str) -> float | None:
    value = value.strip()
    if not value:
        return None
    h, m, s = value.split(":")
    return int(h) + int(m) / 60.0 + float(s) / 3600.0


def parse_dec(value: str) -> float | None:
    value = value.strip()
    if not value:
        return None
    sign = -1.0 if value[0] == "-" else 1.0
    rest = value.lstrip("+-")
    d, m, s = rest.split(":")
    return sign * (int(d) + int(m) / 60.0 + float(s) / 3600.0)


def parse_float(value: str) -> float | None:
    value = value.strip()
    if not value:
        return None
    try:
        return float(value)
    except ValueError:
        return None


NAME_PATTERN = re.compile(r"(NGC|IC)0*(\d+)([A-Z]?)$")


def compact_name(name: str) -> str:
    match = NAME_PATTERN.match(name)
    if not match:
        return name
    prefix, number, suffix = match.group(1), match.group(2), match.group(3)
    return f"{prefix}{int(number)}{suffix}"


def sort_key(name: str) -> tuple[int, int, str]:
    """Stable sort key: prefix bucket, numeric, suffix."""
    match = NAME_PATTERN.match(name)
    if not match:
        return (2, 0, name)
    prefix_bucket = 0 if match.group(1) == "NGC" else 1
    return (prefix_bucket, int(match.group(2)), match.group(3))


def fetch_csv(url: str) -> str:
    print(f"fetching {url}", file=sys.stderr)
    with urllib.request.urlopen(url) as resp:  # noqa: S310 — fixed upstream
        return resp.read().decode("utf-8")


def extract(rows_in: list[dict[str, str]]) -> list[tuple[str, float, float, float, str, float]]:
    out: dict[str, tuple[str, float, float, float, str, float]] = {}
    for row in rows_in:
        name = row["Name"].strip()
        otype = row["Type"].strip()
        if otype in HARD_SKIP_TYPES:
            continue
        if row["M"].strip():
            continue  # covered by messier.csv

        v = parse_float(row["V-Mag"])
        b = parse_float(row["B-Mag"])
        size = parse_float(row["MajAx"]) or 0.0

        if v is not None and b is not None:
            mag: float | None = min(v, b - 0.6)
        elif v is not None:
            mag = v
        elif b is not None:
            mag = b - 0.6
        else:
            mag = None

        keep = False
        if mag is not None and mag <= MAG_CUTOFF:
            keep = True
        elif otype in NEB_TYPES and size >= LARGE_NEBULA_MIN_ARCMIN:
            keep = True
        if not keep:
            continue

        # Only NGC / IC primary names participate in the bright subset.
        # OpenNGC's addendum.csv also exposes Caldwell / ESO / Melotte /
        # MWSC / Hartung entries that lack a renderer-side label scheme;
        # those belong to the PR-B runtime backend which can pass them
        # through with their original designations.
        if not NAME_PATTERN.match(name):
            continue

        ra = parse_ra(row["RA"])
        dec = parse_dec(row["Dec"])
        if ra is None or dec is None:
            continue
        if mag is None:
            mag = NO_PHOTOMETRY_SENTINEL

        kind = KIND_MAP.get(otype, "Other")
        compact = compact_name(name)
        # First occurrence wins; the OpenNGC duplicates carry less metadata.
        out.setdefault(
            compact,
            (compact, ra, dec, round(mag, 2), kind, round(size, 2)),
        )
    return sorted(out.values(), key=lambda row: sort_key(row[0]))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--ngc",
        default=None,
        help="path to a cached OpenNGC NGC.csv (skips download)",
    )
    parser.add_argument(
        "--addendum",
        default=None,
        help="path to a cached OpenNGC addendum.csv (skips download)",
    )
    parser.add_argument(
        "--output",
        default="crates/catalog/data/openngc_bright.csv",
        help="destination CSV path (default: crates/catalog/data/openngc_bright.csv)",
    )
    args = parser.parse_args()

    ngc_text = Path(args.ngc).read_text() if args.ngc else fetch_csv(UPSTREAM_NGC)
    add_text = Path(args.addendum).read_text() if args.addendum else fetch_csv(UPSTREAM_ADDENDUM)

    rows: list[dict[str, str]] = []
    for text in (ngc_text, add_text):
        reader = csv.DictReader(text.splitlines(), delimiter=";")
        rows.extend(reader)

    extracted = extract(rows)
    print(f"kept {len(extracted)} bright NGC/IC entries", file=sys.stderr)

    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("w", encoding="utf-8") as fh:
        fh.write(CSV_HEADER)
        fh.write("name,ra_hours,dec_deg,mag,type,size_arcmin\n")
        for name, ra, dec, mag, kind, size in extracted:
            fh.write(f"{name},{ra:.6f},{dec:.4f},{mag:.2f},{kind},{size:.2f}\n")
    print(f"wrote {out_path}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
