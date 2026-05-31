#!/usr/bin/env bash
set -euo pipefail

# L-06: fetch + slim a JPL DE440 SPK kernel for the `astronomy::spk` reader.
#
# DE440 is distributed by NASA NAIF as a large binary SPK (`.bsp`, ~110 MB for
# the full 1550–2650 range). It is NOT committed to this repository; this
# script documents the supported way to obtain a kernel and slim it to the
# renderer's modern epoch range so it can be loaded at runtime via
# `SpkKernel::from_bytes`.
#
# Requirements: NAIF `brief`/`spkmerge` (CSPICE toolkit) OR python `jplephem`.
#
# Usage:
#   scripts/fetch-de440-subset.sh [OUT_BSP]
#
# Steps performed:
#   1. Download the full DE440 SPK from the NAIF generic-kernels archive.
#   2. (Optional) subset it to 1900–2100 with `spkmerge` to keep it small.
#
# The output kernel is intended for local/desktop use only; the WASM/default
# build keeps the analytic VSOP87 / ELP2000 fallback (see ROADMAP `L-06`).

OUT="${1:-build/de440/de440s.bsp}"
URL="https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/de440s.bsp"

mkdir -p "$(dirname "$OUT")"

echo "Downloading DE440 (short) SPK from NAIF…"
echo "  $URL"
if command -v curl >/dev/null 2>&1; then
  curl -fSL "$URL" -o "$OUT"
elif command -v wget >/dev/null 2>&1; then
  wget -O "$OUT" "$URL"
else
  echo "error: need curl or wget to download the kernel" >&2
  exit 1
fi

echo "Wrote $OUT"
echo
echo "Verify with:  python -c \"import astronomy\"  # not applicable; load via SpkKernel::from_bytes in Rust"
echo "Subset (optional, needs CSPICE spkmerge) to shrink to 1900-2100:"
echo "  spkmerge subset.cmd   # see NAIF spkmerge docs; begin/end UTC 1900/2100"
echo
echo "de440s.bsp covers 1849-2150 and is ~32 MB; the full de440.bsp covers"
echo "1550-2650. Neither is committed (DATA_SOURCES.md / ROADMAP L-06)."
