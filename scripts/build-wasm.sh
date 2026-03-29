#!/bin/bash
set -e
cd "$(dirname "$0")/.."

echo "Building WASM package..."
wasm-pack build apps/web --target web --out-dir frontend/pkg

echo "Installing frontend dependencies..."
cd apps/web/frontend
bun install

echo "Done. Run 'cd apps/web/frontend && bun run dev' to start the dev server."
