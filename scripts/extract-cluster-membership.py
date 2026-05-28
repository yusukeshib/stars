#!/usr/bin/env python3
"""Regenerate ``crates/catalog/data/cluster_membership.csv``.

The V-53 first slice (this script's current mode) emits the hand-curated
showpiece-cluster bootstrap directly. The follow-up upgrade path is wired
in but disabled by default: passing ``--from-cantat-gaudin`` fetches
Cantat-Gaudin 2020 (A&A 633, A99) via VizieR, filters to the showpiece
clusters, and emits a deeper member list with the same column shape.

The committed CSV's SHA-256 is pinned in ``data/manifest.toml``; running
this script with no flags and re-committing must reproduce the same bytes
unless the bootstrap list itself changes.

References
----------
* Cantat-Gaudin, T. et al. 2020, A&A 633, A99, "Painting a portrait of the
  Galactic disc with its stellar clusters". VizieR catalogue
  ``J/A+A/633/A99``.
* Mermilliod, J.-C. & Paunzen, E. 2003, "Open Clusters and the WEBDA
  Database". A&A 410, 511.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# ---------------------------------------------------------------------------
# Hand-curated showpiece bootstrap (V-53 first slice).
# ---------------------------------------------------------------------------
#
# Each tuple is (cluster_id, hyg_id, hip_id_or_None, common_or_bayer_name).
# HYG / HIP IDs were read straight from
# ``crates/catalog/data/hyg_v42.csv`` for stars whose published common /
# Bayer / Flamsteed name matches the cluster's well-known membership list:
# Pleiades 9 named stars, Praesepe core stars at V <= 6.9, Double Cluster
# bright HYG entries split by RA. See the CSV header for the per-cluster
# rationale.
BOOTSTRAP_ROWS: list[tuple[str, int, int | None, str]] = [
    # Pleiades (M45) -- 9 named members; Hipparcos IDs from HYG v4.2.
    ("M45", 17447, 17489, "Celaeno"),
    ("M45", 17457, 17499, "Electra"),
    ("M45", 17489, 17531, "Taygeta"),
    ("M45", 17532, 17573, "Maia"),
    ("M45", 17537, 17579, "Asterope"),
    ("M45", 17566, 17608, "Merope"),
    ("M45", 17661, 17702, "Alcyone"),
    ("M45", 17805, 17847, "Atlas"),
    ("M45", 17809, 17851, "Pleione"),
    # Praesepe / Beehive (M44) -- 11 bright core members at V <= 6.9.
    ("M44", 42208, 42327, ""),
    ("M44", 42366, 42485, ""),
    ("M44", 42377, 42497, ""),
    ("M44", 42397, 42516, "39 Cnc"),
    ("M44", 42404, 42523, "40 Cnc"),
    ("M44", 42423, 42542, ""),
    ("M44", 42429, 42549, ""),
    ("M44", 42437, 42556, "Meleph"),
    ("M44", 42459, 42578, "42 Cnc"),
    ("M44", 42481, 42600, ""),
    ("M44", 42554, 42673, ""),
    # Double Cluster -- NGC 869 (h Per) bright members at HYG depth.
    ("NGC869", 10591, 10615, ""),
    ("NGC869", 10600, 10624, ""),
    ("NGC869", 10609, 10633, ""),
    ("NGC869", 10617, 10641, ""),
    ("NGC869", 10678, 10704, ""),
    ("NGC869", 10703, 10729, ""),
    ("NGC869", 10779, 10805, ""),
    ("NGC869", 10790, 10816, ""),
    ("NGC869", 10847, 10873, ""),
    # Double Cluster -- NGC 884 (chi Per) bright members at HYG depth.
    ("NGC884", 10992, 11018, ""),
    ("NGC884", 10994, 11020, ""),
    ("NGC884", 11072, 11098, ""),
    ("NGC884", 11089, 11115, ""),
    ("NGC884", 11119, 11146, ""),
]

CSV_HEADER = """# Open-cluster membership: showpiece bootstrap slice (V-53).
#
# Membership table that joins HYG / Hipparcos IDs to a parent open cluster's
# deep-sky catalogue ID. The renderer uses this to flag clusters that should
# be drawn as a *resolved field of stars* rather than a single DSO marker:
# the Messier / NGC marker geometry is suppressed for tagged clusters while
# the label is kept, and the underlying HYG stars carry the visible.
#
# Scope (deliberate, see PR V-53):
#   - Pleiades (M45):       9 named members (Alcyone, Atlas, Electra, Maia,
#                           Merope, Taygeta, Pleione, Asterope, Celaeno).
#   - Praesepe (M44):       11 brightest core members at V <= 6.9.
#   - Double Cluster:       NGC 869 (h Per) and NGC 884 (chi Per) bright
#                           members in HYG (HYG only resolves these clusters
#                           down to ~ mag 8.5 because the photometric core
#                           is fainter than HYG's V <= 9 cut). Split by RA:
#                           RA < 2.345 -> NGC 869, RA >= 2.345 -> NGC 884.
#   - Hyades (Mel 25) is intentionally deferred: it has no current V-42 DSO
#     marker to suppress (no Messier number, not in openngc_bright). The
#     Cantat-Gaudin follow-up will add it once a Hyades label asset lands.
#
# Provenance:
#   The 4 showpiece clusters here are bootstrapped from each cluster's
#   well-documented bright-named-member list (HYG ID + Bayer / common name)
#   so the deliverable for V-53 lands without a VizieR download dependency.
#   A follow-up PR will replace this hand-curated slice with a deterministic
#   extraction from the Cantat-Gaudin 2020 (A&A 633, A99) Gaia DR2/DR3
#   membership catalog; see scripts/extract-cluster-membership.py for the
#   stub that future extraction will fill in.
#
# Columns:
#   cluster_id - DSO identifier in `M<N>` / `NGC<N>` / `IC<N>` form. Must
#                match a row in messier.csv or openngc_bright.csv (whichever
#                catalogue owns the cluster's marker geometry today).
#   hyg_id     - HYG v4.2 `id` column. Required.
#   hip_id     - Hipparcos catalogue number when present in HYG. Empty when
#                the star has no Hipparcos cross-id. Used by future
#                Cantat-Gaudin joins that key on HIP.
#   name       - Human-readable common / Bayer / Flamsteed name. Empty when
#                HYG carries no name. Informational only; the renderer does
#                not use it.
"""


def write_csv(out_path: Path, rows: list[tuple[str, int, int | None, str]]) -> None:
    """Emit ``rows`` to ``out_path`` with the documented header."""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    with out_path.open("w", encoding="utf-8") as fh:
        fh.write(CSV_HEADER)
        fh.write("cluster_id,hyg_id,hip_id,name\n")
        for cluster_id, hyg, hip, name in rows:
            hip_str = "" if hip is None else str(hip)
            fh.write(f"{cluster_id},{hyg},{hip_str},{name}\n")


def regenerate_from_cantat_gaudin() -> list[tuple[str, int, int | None, str]]:
    """Placeholder follow-up path.

    Future implementation will:
      1. Download VizieR ``J/A+A/633/A99`` table ``members.dat`` (probability
         column ``Proba``, Gaia DR2 source IDs, HIP cross-ids).
      2. Restrict to ``Proba >= 0.7`` for the showpiece cluster slate.
      3. Join HIP to HYG v4.2 ``hip`` column for member rows already in HYG.
      4. Sort by ``(cluster_id, hyg_id)`` and emit through ``write_csv``.

    The V-53 bootstrap CSV intentionally avoids this path so the first slice
    has no network dependency at make-time.
    """
    raise SystemExit(
        "Cantat-Gaudin follow-up extraction not yet implemented; see V-53 PR."
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        default="crates/catalog/data/cluster_membership.csv",
        help="destination CSV path (default: crates/catalog/data/cluster_membership.csv)",
    )
    parser.add_argument(
        "--from-cantat-gaudin",
        action="store_true",
        help="(NOT YET IMPLEMENTED) extract from Cantat-Gaudin 2020 via VizieR",
    )
    args = parser.parse_args()

    if args.from_cantat_gaudin:
        rows = regenerate_from_cantat_gaudin()
    else:
        rows = BOOTSTRAP_ROWS

    out_path = Path(args.output)
    write_csv(out_path, rows)
    print(f"wrote {out_path} ({len(rows)} member rows)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
