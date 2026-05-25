#!/usr/bin/env bash
set -euo pipefail

# Render or check the validation/demo gallery backed by built-in scene presets.
#
# Usage:
#   ./scripts/render-validation-gallery.sh          # render/update docs/assets/validation/*.png
#   ./scripts/render-validation-gallery.sh --check  # render to a temp dir and byte-compare to committed PNGs
#
# The --check mode is intentionally opt-in rather than part of default CI:
# wgpu readback can vary by adapter/driver. Projects with pinned CI GPU/SwiftShader
# images can call --check to enable exact screenshot regression.

mode="update"
out_dir="docs/assets/validation"
width="${STARS_VALIDATION_WIDTH:-480}"
height="${STARS_VALIDATION_HEIGHT:-270}"

if [[ "${1:-}" == "--check" ]]; then
  mode="check"
elif [[ "${1:-}" == "--update" || "${1:-}" == "" ]]; then
  mode="update"
else
  echo "usage: $0 [--update|--check]" >&2
  exit 2
fi

mapfile -t presets < <(cargo run -p stars-cli -- --list-presets | awk '{print $1}')

if [[ "${mode}" == "check" ]]; then
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${tmp_dir}"' EXIT
  target_dir="${tmp_dir}"
else
  mkdir -p "${out_dir}"
  target_dir="${out_dir}"
fi

for preset in "${presets[@]}"; do
  cargo run -p stars-cli --release -- \
    --preset "${preset}" \
    --width "${width}" \
    --height "${height}" \
    -o "${target_dir}/${preset}.png"
  echo "rendered ${target_dir}/${preset}.png"
done

if [[ "${mode}" == "check" ]]; then
  for preset in "${presets[@]}"; do
    expected="${out_dir}/${preset}.png"
    actual="${target_dir}/${preset}.png"
    if [[ ! -f "${expected}" ]]; then
      echo "missing baseline ${expected}; run $0 --update and review/commit the result" >&2
      exit 1
    fi
    if ! cmp -s "${expected}" "${actual}"; then
      echo "visual regression for ${preset}: ${actual} differs from ${expected}" >&2
      exit 1
    fi
  done
  echo "validation gallery matches committed baselines"
fi
