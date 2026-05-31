#!/usr/bin/env python3
"""Regenerate ``crates/catalog/data/double_stars.csv``.

The V-54 first slice (this script's current mode) emits the hand-curated
Washington Double Star (WDS) showpiece bootstrap directly. The follow-up
upgrade path is wired in but disabled by default: passing ``--from-wds``
will query the WDS for the showpiece pairs (and, once the large-catalog
ingest lands, every HYG primary with a WDS companion) and emit a deeper
table with the same column shape.

The committed CSV's SHA-256 is pinned in ``data/manifest.toml``; running
this script with no flags and re-committing must reproduce the same bytes
unless the bootstrap list itself changes.

References
----------
* Mason, B. D., Wycoff, G. L., Hartkopf, W. I., Douglass, G. G.,
  Worley, C. E. 2001, AJ 122, 3466, "The 2001 US Naval Observatory Double
  Star CD-ROM. I. The Washington Double Star Catalog". DOI 10.1086/323920.
  Catalog home: http://www.astro.gsu.edu/wds/
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Hand-curated WDS showpiece bootstrap (V-54 first slice).
# ---------------------------------------------------------------------------
#
# Each tuple matches the committed CSV column order:
#   (name, hyg_id, ra_hours, dec_deg, rho_arcsec, theta_deg,
#    m1, m2, bv1, bv2, epoch, wds_id)
#
# Separation / position angle / component magnitudes are read from the WDS at
# the listed epoch; the HYG id and RA/Dec come from
# ``crates/catalog/data/hyg_v42.csv`` for the merged primary row the pair
# resolves. Pairs HYG already ships as two distinct rows are intentionally
# omitted to avoid double-counting: Albireo (beta-1 / beta-2 Cyg), Castor
# (alpha Gem A = id 36744, B = id 118485), and -- crucially -- Mizar, whose
# B component is already HYG id 118887 at ~19" (and Alcor is id 65272 at ~12').
BOOTSTRAP_ROWS: list[tuple] = [
    ("Algieba AB (gamma Leo)", 50440, 10.332873, 19.841489, 4.6, 126.0, 2.37, 3.64, 1.13, 1.15, 2015.0, "10200+1950"),
    ("Epsilon-1 Lyrae AB", 91633, 18.738984, 39.670123, 2.35, 347.0, 5.06, 6.02, 0.17, 0.20, 2018.0, "18443+3940"),
    ("Epsilon-2 Lyrae CD", 91639, 18.739661, 39.612721, 2.40, 80.0, 5.14, 5.37, 0.18, 0.20, 2018.0, "18443+3937"),
]

HEADER_COMMENT = """\
# Washington Double Star (WDS) showpiece bootstrap for V-54.
#
# Each row is a visual double / binary whose components HYG v4.2 merges into a
# single catalog entry (or, for the epsilon Lyrae "Double Double", two entries
# that are each themselves an unresolved pair). At eyepiece zoom the renderer
# splits the keyed HYG primary into two component sprites placed at the WDS
# separation rho (arcsec) and position angle theta (degrees, North through
# East), with each component carrying its own B-V colour through the V-23
# blackbody pipeline.
#
# Columns:
#   name        informational label (ignored by the renderer)
#   hyg_id      HYG v4.2 `id` of the merged primary row this pair resolves
#   ra_hours    primary J2000 right ascension (hours) for position matching
#   dec_deg     primary J2000 declination (degrees) for position matching
#   rho_arcsec  WDS angular separation of the secondary from the primary
#   theta_deg   WDS position angle (deg, measured North -> East)
#   m1          primary component V magnitude
#   m2          secondary component V magnitude
#   bv1         primary component B-V colour index
#   bv2         secondary component B-V colour index
#   epoch       WDS measurement epoch (Besselian year)
#   wds_id      WDS catalog identifier (Mason et al. 2001)
#
# Pairs already shipped as two distinct HYG rows are deliberately omitted: HYG
# already resolves Albireo (beta-1/beta-2 Cyg), Castor (alpha Gem A/B), and
# Mizar (A = id 65173, B = id 118887, Alcor = id 65272), so adding them here
# would double-count.
"""

COLUMNS = "name,hyg_id,ra_hours,dec_deg,rho_arcsec,theta_deg,m1,m2,bv1,bv2,epoch,wds_id"


def render_csv() -> str:
    lines = [HEADER_COMMENT.rstrip("\n"), COLUMNS]
    for row in BOOTSTRAP_ROWS:
        lines.append(",".join(str(field) for field in row))
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--from-wds",
        action="store_true",
        help="(stub) query the WDS for a deeper table; not yet implemented.",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "crates"
        / "catalog"
        / "data"
        / "double_stars.csv",
    )
    args = parser.parse_args()

    if args.from_wds:
        print(
            "--from-wds is stubbed for the full-catalog ingest follow-up; "
            "the committed CSV is the hand-curated bootstrap.",
            file=sys.stderr,
        )
        return 2

    args.out.write_text(render_csv(), encoding="utf-8")
    print(f"wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
