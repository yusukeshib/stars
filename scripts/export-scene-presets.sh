#!/usr/bin/env bash
set -euo pipefail

# Export every built-in deterministic scene preset as a schema-versioned JSON
# session. Run from the repository root.

out_dir="${1:-docs/presets/sessions}"
mkdir -p "${out_dir}"

mapfile -t presets < <(cargo run -p stars-cli -- --list-presets | awk '{print $1}')

for preset in "${presets[@]}"; do
  cargo run -p stars-cli -- \
    --preset "${preset}" \
    --write-session "${out_dir}/${preset}.json" \
    --write-session-only
  echo "wrote ${out_dir}/${preset}.json"
done
