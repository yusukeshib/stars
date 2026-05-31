#!/usr/bin/env bash
set -euo pipefail

# Fetch a magnitude-cut Gaia DR3 subset as CSV that crates/catalog
# `GaiaDr3CsvBackend` parses by column name. The full Gaia DR3 source is ~1.8
# BILLION rows and cannot be embedded or even fully downloaded here; this pulls
# only a bright all-sky tier (default phot_g_mean_mag <= 8). The export is
# written under build/ and is NOT committed. Streaming LOD paging of the full
# source is the L-17 follow-up.
#
# Source / citation:
#   Gaia Collaboration 2022, A&A 674, A1 (Gaia DR3).
#   Gaia archive ESA TAP (https://gea.esac.esa.int/tap-server/tap).
#   License: Gaia data are released under CC-BY-SA 3.0 IGO with the standard
#   Gaia acknowledgement (https://www.cosmos.esa.int/web/gaia-users/credits).
#
# Usage:
#   scripts/fetch-gaia-dr3-subset.sh [OUT_CSV] [G_MAG_LIMIT]
# OUT_CSV defaults to build/catalogs/gaia_dr3_bright.csv; G_MAG_LIMIT to 8.0.

out_csv="${1:-build/catalogs/gaia_dr3_bright.csv}"
g_limit="${2:-8.0}"
tap="https://gea.esac.esa.int/tap-server/tap/sync"
adql="SELECT source_id, ra, dec, parallax, pmra, pmdec, phot_g_mean_mag, bp_rp FROM gaiadr3.gaia_source WHERE phot_g_mean_mag <= ${g_limit}"

mkdir -p "$(dirname "${out_csv}")"
echo "Fetching Gaia DR3 bright subset (G <= ${g_limit}) -> ${out_csv}" >&2

if command -v curl >/dev/null 2>&1; then
  curl -fL --retry 3 \
    --data-urlencode "REQUEST=doQuery" \
    --data-urlencode "LANG=ADQL" \
    --data-urlencode "FORMAT=csv" \
    --data-urlencode "QUERY=${adql}" \
    "${tap}" \
    -o "${out_csv}"
else
  echo "error: need curl to download" >&2
  exit 1
fi

echo "Done. Rows: $(($(wc -l < "${out_csv}") - 1)). Load via catalog::GaiaDr3CsvBackend." >&2
