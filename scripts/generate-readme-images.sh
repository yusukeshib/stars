#!/usr/bin/env bash
set -euo pipefail

# Generate the README gallery images from deterministic CLI scenes.
# Run from the repository root after `make setup` has downloaded the HYG catalog.

out_dir="docs/assets/readme"
mkdir -p "${out_dir}"

cargo run -p stars-cli --release -- \
  --lat 35.68 \
  --lng 139.69 \
  --time 2026-08-13T12:00:00Z \
  --azimuth 180 \
  --altitude 35 \
  --fov 75 \
  --width 1920 \
  --height 1080 \
  --overlays horizon,cardinals,cardinal-labels,ecliptic,galactic-equator,planet-labels \
  -o "${out_dir}/tokyo-summer-sky.png"

cargo run -p stars-cli --release -- \
  --lat 35.68 \
  --lng 139.69 \
  --time 2026-08-13T12:00:00Z \
  --projection hammer \
  --width 1920 \
  --height 1080 \
  --overlays equatorial-grid,ecliptic,galactic-equator \
  -o "${out_dir}/hammer-all-sky.png"

cargo run -p stars-cli --release -- \
  --lat 35.68 \
  --lng 139.69 \
  --time 2026-08-13T12:00:00Z \
  --viewpoint galactic-north \
  --projection perspective \
  --width 1920 \
  --height 1080 \
  --no-overlays \
  -o "${out_dir}/galactic-north.png"
