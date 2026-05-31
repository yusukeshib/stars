#!/usr/bin/env bash
set -euo pipefail

# Fetch a Tycho-2 (VizieR I/259/tyc2) CSV export that crates/catalog
# `Tycho2CsvBackend` parses by column name. The full catalogue is ~2.5M rows
# (~250 MB); fetching the whole thing is gated behind an explicit magnitude cut
# so the default pull stays manageable. The export is written under build/ and
# is NOT committed.
#
# Source / citation:
#   Høg, E. et al. 2000, A&A 355, L27.
#   VizieR catalogue I/259 (https://vizier.cds.unistra.fr/viz-bin/VizieR?-source=I/259).
#   License: CDS/VizieR terms (free for research with attribution).
#
# Usage:
#   scripts/fetch-tycho2.sh [OUT_CSV] [VT_MAG_LIMIT]
# OUT_CSV defaults to build/catalogs/tycho2.csv; VT_MAG_LIMIT defaults to 8.0.

out_csv="${1:-build/catalogs/tycho2.csv}"
vt_limit="${2:-8.0}"
tap="https://tapvizier.cds.unistra.fr/TAPVizieR/tap/sync"
adql="SELECT TYC1, TYC2, TYC3, RAmdeg, DEmdeg, BTmag, VTmag, pmRA, pmDE, HIP FROM \"I/259/tyc2\" WHERE VTmag <= ${vt_limit}"

mkdir -p "$(dirname "${out_csv}")"
echo "Fetching Tycho-2 (VizieR I/259, VTmag <= ${vt_limit}) -> ${out_csv}" >&2

if command -v curl >/dev/null 2>&1; then
  curl -fL --retry 3 -G "${tap}" \
    --data-urlencode "REQUEST=doQuery" \
    --data-urlencode "LANG=ADQL" \
    --data-urlencode "FORMAT=csv" \
    --data-urlencode "QUERY=${adql}" \
    -o "${out_csv}"
else
  echo "error: need curl to download" >&2
  exit 1
fi

echo "Done. Rows: $(($(wc -l < "${out_csv}") - 1)). Load via catalog::Tycho2CsvBackend." >&2
