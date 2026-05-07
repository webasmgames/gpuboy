#!/usr/bin/env bash
set -euo pipefail

echo "==> cargo fmt -- --check"
cargo fmt -- --check

echo "==> cargo clippy"
cargo clippy -- -D warnings

echo "==> cargo test"
cargo test

echo "==> wasm-pack build crates/gpuboy-wasm --target web"
wasm-pack build crates/gpuboy-wasm --target web --out-dir ../../pkg

echo ""
echo "PASS"
