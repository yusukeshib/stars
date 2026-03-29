.PHONY: native web web-build web-dev setup fmt clippy test ci clean

# Native app
native:
	cargo run -p stars-native --release

# Web app (build WASM + start dev server)
web: web-build web-dev

web-build:
	wasm-pack build apps/web --target web --out-dir frontend/pkg

web-dev:
	cd apps/web/frontend && npm run dev

# Download star catalog
setup:
	./scripts/download-catalog.sh
	cd apps/web/frontend && npm install --cache /tmp/npm-cache

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
