#!/usr/bin/env bash
# Builds the web version into dist/. Serve it with any static server, e.g.
#   python3 -m http.server -d dist 8000
set -euo pipefail
cd "$(dirname "$0")"
cargo build --profile web --target wasm32-unknown-unknown
rm -rf dist && mkdir -p dist
cp -R web/. dist/
cp target/wasm32-unknown-unknown/web/grumpy-pigs.wasm dist/
ls -lh dist
