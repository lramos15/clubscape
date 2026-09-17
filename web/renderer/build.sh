#!/usr/bin/env bash
# Builds the Rust/WASM renderer, generates wasm-bindgen glue into web/renderer/pkg and compiles
# the TypeScript adapter + developer fixture page into web/renderer/dist.
# Requires: Rust with target wasm32-unknown-unknown, wasm-bindgen CLI 0.2.128 (must equal the
# wasm-bindgen crate pin in crates/renderer/Cargo.toml), web/node_modules (pnpm install in web/).
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$repo"
cargo build -p clubscape-renderer --release --features web --target wasm32-unknown-unknown
expected="0.2.128"
actual="$(wasm-bindgen --version | awk '{print $2}')"
if [[ "$actual" != "$expected" ]]; then
  echo "wasm-bindgen CLI $actual does not match crate pin $expected" >&2
  exit 1
fi
wasm-bindgen --target web --out-dir web/renderer/pkg --out-name clubscape_renderer \
  target/wasm32-unknown-unknown/release/clubscape_renderer.wasm
if command -v wasm-opt >/dev/null 2>&1 && [[ "${CLUBSCAPE_WASM_OPT:-1}" == "1" ]]; then
  wasm-opt -O2 --enable-bulk-memory --enable-nontrapping-float-to-int \
    web/renderer/pkg/clubscape_renderer_bg.wasm -o web/renderer/pkg/clubscape_renderer_bg.wasm
fi
web/node_modules/.bin/tsc -p web/renderer/tsconfig.json
mkdir -p web/renderer/dist/renderer/pkg
cp web/renderer/pkg/clubscape_renderer.js web/renderer/pkg/clubscape_renderer_bg.wasm web/renderer/dist/renderer/pkg/
sha256sum web/renderer/pkg/clubscape_renderer_bg.wasm web/renderer/pkg/clubscape_renderer.js
