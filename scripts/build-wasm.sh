#!/bin/bash
set -e
cd "$(dirname "$0")/.."

echo "Building WASM package..."
wasm-pack build apps/web --target web --out-dir frontend/pkg

echo "Installing frontend dependencies..."
cd apps/web/frontend
npm install

echo "Done. Run 'cd apps/web/frontend && npm run dev' to start the dev server."
