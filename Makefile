.PHONY: cli viewer server dev web web-build web-install web-dev frontend-check setup scene-presets validation-gallery validation-gallery-check demo-gallery demo-gallery-check notebook-check manifest-check pyo3-check fmt clippy test ci clean

# PNG-output CLI (override ARGS, e.g. `make cli ARGS="--lat 35.68 --lng 139.69 -o /tmp/sky.png"`).
ARGS ?= --lat 35.68 --lng 139.69 --azimuth 180 --altitude 30 -o stars.png
cli:
	cargo run -p stars-cli --release -- $(ARGS)

# Interactive desktop viewer.
viewer:
	cargo run -p stars-viewer --release

# Headless HTTP host (L-22). POST a session JSON to `/render` for a PNG;
# `GET /healthz` and `GET /presets[/<id>]` are available too.
SERVER_ARGS ?= --port 8787
server:
	cargo run -p stars-server --release -- $(SERVER_ARGS)

# Web app (build WASM + install workspace deps + start dev server)
dev: web

web: web-build web-install web-dev

web-build:
	wasm-pack build apps/web --target web --out-dir frontend/pkg

web-install:
	cd apps/web/frontend && bun install

web-dev:
	cd apps/web/frontend && bun run dev

frontend-check:
	cd apps/web/frontend && bun install --frozen-lockfile
	cd apps/web/frontend && bun run tsc --noEmit

# Download star catalog
setup: web-build web-install
	./scripts/download-catalog.sh

# Export deterministic JSON sessions for built-in scene presets.
scene-presets:
	./scripts/export-scene-presets.sh

# Render/update the validation/demo gallery PNGs.
validation-gallery:
	./scripts/render-validation-gallery.sh --update

# Opt-in exact screenshot regression for pinned GPU/driver environments.
validation-gallery-check:
	./scripts/render-validation-gallery.sh --check

# Render/update the curated public demo gallery (L-14).
demo-gallery:
	./scripts/render-demo-gallery.sh --update

# Opt-in exact screenshot regression for the demo gallery on pinned GPUs.
demo-gallery-check:
	./scripts/render-demo-gallery.sh --check

# Check notebook-backed astronomy table fixtures without requiring Jupyter or a GPU.
notebook-check:
	python3 examples/notebooks/session_reproducibility.py --check-tables

# Verify data/manifest.toml against on-disk bytes (P3-13 data provenance manifest).
manifest-check:
	cargo run -q -p stars-manifest --bin check-manifest

# L-21 PyO3 binding: type-check the wrapper crate without requiring a Python
# toolchain. The wheel build (`maturin develop --features extension-module`)
# is documented in bindings/python/README.md and is not part of `make ci`
# yet — the cargo check below + the binding's pure-Rust unit tests are the
# present gate.
pyo3-check:
	cargo check -p stars-py

# Lint & test
fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test --workspace

ci: fmt
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -D warnings
	cargo test --workspace
	$(MAKE) manifest-check
	$(MAKE) notebook-check
	cargo check -p stars-web --target wasm32-unknown-unknown --manifest-path apps/web/Cargo.toml
	$(MAKE) frontend-check
	$(MAKE) pyo3-check

# Clean
clean:
	cargo clean
	rm -rf apps/web/frontend/pkg apps/web/target
