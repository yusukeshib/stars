#!/usr/bin/env bash
#
# Regenerate `crates/astronomy/data/redtass7.dat` — the embedded coefficient
# table backing the `V-52c-TASS17` Titan precision upgrade (Vienne & Duriez
# 1995 TASS1.7 theory).
#
# The IMCCE distribution ships a single Fortran file `tass17.f` that bundles
# both the evaluation subroutines (POSIRED / CALCLON / CALCELEM / EDERED /
# LECSER) and the numeric series. The README instructs splitting the file
# into a `POSIRED7.FOR` (code) part and a `redtass7.dat` (series) part. This
# script downloads `tass17.f` and extracts exactly the series block —
# starting at the line `0.01720209895D0` (the Gauss constant header) and
# ending at the closing `9999` sentinel of satellite 8 (Iapetus), i.e. the
# satellites Mimas..Titan + Iapetus that our Titan evaluator needs.
#
# Hyperion (TASS satellite index 7) is deliberately excluded: it uses a
# different on-disk format (read by the upstream `LITHYP`) and its proper
# longitude `DLO(7)` is fixed at 0 by `CALCLON`, so omitting it leaves the
# Titan result bit-for-bit unchanged while keeping the parser simple.
#
# Upstream:
#   ftp://ftp.imcce.fr/pub/ephem/satel/tass17/tass17.f
#   ftp://ftp.imcce.fr/pub/ephem/satel/tass17/README
#   Vienne, A. & Duriez, L. 1995, A&A 297, 588.
#
# Usage:
#   bash scripts/build-tass17.sh
#
# Determinism: the IMCCE TASS1.7 distribution is frozen (dated 1996/2005);
# the extracted bytes are stable, so the SHA-256 pinned in data/manifest.toml
# does not drift across reruns.

set -euo pipefail

cd "$(dirname "$0")/.."

OUT="crates/astronomy/data/redtass7.dat"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

SRC="$TMP/tass17.f"
curl -sSL --fail -o "$SRC" "ftp://ftp.imcce.fr/pub/ephem/satel/tass17/tass17.f"

# Extract the SERIES block: from the Gauss-constant header line down to and
# including the last `9999` sentinel that precedes the Hyperion block
# (`  7   0`). We anchor on content rather than fixed line numbers so the
# extraction survives incidental whitespace edits in future IMCCE releases.
awk '
    # Begin capturing at the Gauss constant that the README says the file
    # "must begin by".
    /^[[:space:]]*0\.01720209895D0[[:space:]]*$/ { capture = 1 }
    # Stop *before* the Hyperion satellite block header "  7   0".
    capture && /^[[:space:]]*7[[:space:]]+0[[:space:]]*$/ { exit }
    capture { print }
' "$SRC" > "$OUT"

lines=$(wc -l < "$OUT")
if [ "$lines" -lt 1000 ]; then
    echo "extraction produced only $lines lines — upstream format may have changed" >&2
    exit 1
fi

echo "wrote $lines lines to $OUT"
echo "sha256: $(shasum -a 256 "$OUT" | awk '{print $1}')"
