#!/usr/bin/env bash
set -euo pipefail

# Download the Falchi et al. 2016 "World Atlas of Artificial Night Sky
# Brightness" GeoTIFF (the 2015 data release). The raster is ~1 GB and is NOT
# committed to this repository; this fetch + `build-falchi-atlas.py` are the
# supported path for the V-39-Atlas light-pollution loader.
#
# Source / citation:
#   Falchi, F. et al. 2016, "The new world atlas of artificial night sky
#   brightness", Science Advances 2, e1600377, DOI 10.1126/sciadv.1600377.
#   Data release: GFZ Data Services, DOI 10.5880/GFZ.1.4.2016.001.
#
# The exact direct download URL for the GeoTIFF lives behind the DOI landing
# page and changes between mirrors, so this script does NOT hard-code a guessed
# link. Provide the resolved GeoTIFF URL explicitly:
#
#   FALCHI_ATLAS_URL="https://<resolved-geotiff-url>" \
#     scripts/fetch-falchi-atlas.sh [OUT_TIF]
#
# or pass it as the second argument. OUT_TIF defaults to
# `build/falchi/World_Atlas_2015.tif`.

DOI_PAGE="https://doi.org/10.5880/GFZ.1.4.2016.001"
out_tif="${1:-build/falchi/World_Atlas_2015.tif}"
url="${FALCHI_ATLAS_URL:-${2:-}}"

if [[ -z "${url}" ]]; then
  cat >&2 <<EOF
No GeoTIFF URL supplied.

1. Open the data release landing page and accept its licence terms:
     ${DOI_PAGE}
2. Copy the direct download URL for the World Atlas 2015 GeoTIFF.
3. Re-run with:
     FALCHI_ATLAS_URL="<that-url>" scripts/fetch-falchi-atlas.sh "${out_tif}"

Then build the compact grid:
     scripts/build-falchi-atlas.py "${out_tif}" data-cache/falchi_atlas.bin
     export STARS_FALCHI_ATLAS="\$(pwd)/data-cache/falchi_atlas.bin"
EOF
  exit 2
fi

mkdir -p "$(dirname "${out_tif}")"
echo "Downloading Falchi 2016 World Atlas GeoTIFF -> ${out_tif}" >&2
echo "  source: ${url}" >&2
echo "  citation: Falchi et al. 2016, Sci Adv 2 e1600377 (${DOI_PAGE})" >&2

if command -v curl >/dev/null 2>&1; then
  curl -fL --retry 3 -o "${out_tif}" "${url}"
elif command -v wget >/dev/null 2>&1; then
  wget -O "${out_tif}" "${url}"
else
  echo "error: need curl or wget to download" >&2
  exit 1
fi

echo "Done. Next: scripts/build-falchi-atlas.py ${out_tif} data-cache/falchi_atlas.bin" >&2
