.PHONY: cli viewer dev web web-build web-install web-dev setup fmt clippy test ci clean

# PNG-output CLI (override ARGS, e.g. `make cli ARGS="--lat 35.68 --lng 139.69 -o /tmp/sky.png"`).
ARGS ?= --lat 35.68 --lng 139.69 --azimuth 180 --altitude 30 -o stars.png
cli:
	cargo run -p stars-cli --release -- $(ARGS)

# Interactive desktop viewer.
viewer:
	cargo run -p stars-viewer --release

# Web app (build WASM + install workspace deps + start dev server)
dev: web

web: web-build web-install web-dev

web-build:
	wasm-pack build apps/web --target web --out-dir frontend/pkg

web-install:
	bun install

web-dev:
	bun run dev

# Download star catalog
setup: web-build web-install
	./scripts/download-catalog.sh

# Lint & test
fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test

ci: fmt
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -D warnings
	cargo test
	cargo check -p stars-web --target wasm32-unknown-unknown --manifest-path apps/web/Cargo.toml

# Clean
clean:
	cargo clean
	rm -rf apps/web/frontend/pkg apps/web/target
