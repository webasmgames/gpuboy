#!/usr/bin/env bash
set -euo pipefail

echo "==> cargo fmt -- --check"
cargo fmt -- --check

echo "==> cargo clippy"
cargo clippy -- -D warnings

echo "==> cargo test"
cargo test

echo "==> wasm-pack build crates/gpuboy-wasm --target web --release"
wasm-pack build crates/gpuboy-wasm --target web --release --out-dir ../../pkg

echo "==> esbuild (minify)"
mkdir -p dist/www
esbuild www/index.js --minify --outfile=dist/www/index.js
esbuild www/style.css --minify --outfile=dist/www/style.css

echo ""
echo "PASS"
