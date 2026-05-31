#!/usr/bin/env bash
set -euo pipefail

# Fetch the Hipparcos main catalogue (VizieR I/239/hip_main) as a normalised
# CSV that crates/catalog `HipparcosCsvBackend` parses by column name. The
# ~118k-row export (~10 MB) is written under build/ and is NOT committed; the
# committed bright-star anchor index (scripts/extract-bright-star-xmatch.py)
# is the only catalogue subset checked in.
#
# Source / citation:
#   Perryman, M. A. C. et al. 1997, A&A 323, L49.
#   ESA 1997, The Hipparcos and Tycho Catalogues, ESA SP-1200.
#   VizieR catalogue I/239 (https://vizier.cds.unistra.fr/viz-bin/VizieR?-source=I/239).
#   License: CDS/VizieR terms (free for research with attribution).
#
# Usage:
#   scripts/fetch-hipparcos.sh [OUT_CSV]
# OUT_CSV defaults to build/catalogs/hipparcos.csv.

out_csv="${1:-build/catalogs/hipparcos.csv}"
tap="https://tapvizier.cds.unistra.fr/TAPVizieR/tap/sync"
adql="SELECT HIP, RAICRS, DEICRS, Vmag, Plx, pmRA, pmDE, \"B-V\", HD FROM \"I/239/hip_main\""

mkdir -p "$(dirname "${out_csv}")"
echo "Fetching Hipparcos main catalogue (VizieR I/239) -> ${out_csv}" >&2

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

echo "Done. Rows: $(($(wc -l < "${out_csv}") - 1)). Load via catalog::HipparcosCsvBackend." >&2
