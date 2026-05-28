#!/usr/bin/env bash
set -euo pipefail

# Render or check the curated public demo gallery (L-14).
#
# Usage:
#   ./scripts/render-demo-gallery.sh           # render/update docs/assets/demo-gallery/*.png
#   ./scripts/render-demo-gallery.sh --update  # same
#   ./scripts/render-demo-gallery.sh --check   # render to a temp dir and byte-compare to committed PNGs
#
# Unlike the validation gallery (which renders every built-in preset), the demo
# gallery renders a curated, narrated subset suitable for the project front
# door. Each scene is reproducible by name with `cargo run -p stars-cli --
# --preset <name>`. The curated list is the single source of truth for which
# scenes appear in `docs/demo-gallery.md`.
#
# `--check` is opt-in (not part of `make ci`) because wgpu readback can vary by
# GPU/driver. The manifest hash gate in `data/manifest.toml` (verified by
# `make manifest-check`, which IS in `make ci`) protects against silent drift
# of the committed bytes regardless of adapter.

mode="update"
out_dir="docs/assets/demo-gallery"
width="${STARS_DEMO_GALLERY_WIDTH:-480}"
height="${STARS_DEMO_GALLERY_HEIGHT:-270}"

if [[ "${1:-}" == "--check" ]]; then
  mode="check"
elif [[ "${1:-}" == "--update" || "${1:-}" == "" ]]; then
  mode="update"
else
  echo "usage: $0 [--update|--check]" >&2
  exit 2
fi

# Curated subset — keep in sync with `docs/demo-gallery.md`.
presets=(
  tokyo-tonight
  sunset
  civil-twilight-antisolar-tokyo
  moonlit-night
  dark-sky
  dark-sky-bortle-1
  tokyo-bortle-8
  solar-eclipse
  venus-transit
  jupiter-shadow-transit
  all-sky-mollweide
  galactic-north
)

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
  echo "demo gallery matches committed baselines"
fi
