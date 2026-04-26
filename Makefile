.PHONY: native web web-build web-dev setup fmt clippy test ci clean

# Native PNG renderer (override ARGS, e.g. `make native ARGS="--lat 35.68 --lng 139.69 -o /tmp/sky.png"`).
ARGS ?= --lat 35.68 --lng 139.69 --azimuth 180 --altitude 30 -o stars.png
native:
	cargo run -p stars-native --release -- $(ARGS)

# Web app (build WASM + start dev server)
web: web-build web-dev

web-build:
	wasm-pack build apps/web --target web --out-dir frontend/pkg

web-dev:
	cd apps/web/frontend && bun run dev

# Download star catalog
setup:
	./scripts/download-catalog.sh
	cd apps/web/frontend && bun install

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
