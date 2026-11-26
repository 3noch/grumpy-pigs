#!/usr/bin/env bash
# Builds the web version into dist/. Serve it with any static server, e.g.
#   python3 -m http.server -d dist 8000
#
# Caching scheme: the heavy files (game, JS) are renamed after a hash of their
# contents, e.g. grumpy-pigs.3f9c2a17bd.wasm, so a given URL never changes and
# can be cached forever. index.html is a tiny loader that names the current
# files; it is the only thing that needs checking for updates.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --profile web --target wasm32-unknown-unknown
rm -rf dist && mkdir -p dist

# Copy everything, then replace the hashable files with fingerprinted copies.
cp -R web/. dist/
cp target/wasm32-unknown-unknown/web/grumpy-pigs.wasm dist/

hash() { shasum -a 256 "$1" | cut -c1-10; }

assets=()
for f in grumpy-pigs.wasm mq_js_bundle.js platform.js; do
  base="${f%.*}" ext="${f##*.}"
  hashed="$base.$(hash "dist/$f").$ext"
  mv "dist/$f" "dist/$hashed"
  # Point index.html at the fingerprinted name.
  sed -i.bak "s|__${f}__|${hashed}|g" dist/index.html
  assets+=("\"$hashed\"")
done
rm -f dist/index.html.bak

# The service worker precaches exactly this build's files. Its build id changes
# whenever any of them (or the loader) changes, which tells browsers to update.
build=$(cat dist/index.html dist/*.wasm dist/*.js | shasum -a 256 | cut -c1-10)
list=$(IFS=,; echo "${assets[*]}")
sed -i.bak -e "s|__BUILD__|$build|" -e "s|^const ASSETS = \[\];|const ASSETS = [$list];|" dist/sw.js
rm -f dist/sw.js.bak

if grep -q "__" dist/index.html dist/sw.js; then
  echo "error: unreplaced placeholder in dist/index.html or dist/sw.js" >&2
  exit 1
fi
ls -lh dist
