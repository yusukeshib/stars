#!/bin/bash
set -e
cd "$(dirname "$0")/.."

DATA_DIR="crates/stars-catalog/data"
CSV_FILE="$DATA_DIR/hyg_v42.csv"

if [ -f "$CSV_FILE" ] && [ "$(wc -l < "$CSV_FILE")" -gt 100 ]; then
    echo "Star catalog already exists at $CSV_FILE"
    exit 0
fi

echo "Downloading HYG v4.2 star catalog..."
mkdir -p "$DATA_DIR"
curl -L -o "$CSV_FILE.gz" \
    "https://codeberg.org/astronexus/hyg/media/branch/main/data/hyg/CURRENT/hyg_v42.csv.gz"
gunzip -f "$CSV_FILE.gz"
echo "Downloaded $(wc -l < "$CSV_FILE") rows to $CSV_FILE"
