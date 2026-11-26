#!/usr/bin/env bash
# Regenerates the app icons in web/icons/ by rendering them with the game itself.
# macOS only (uses `sips`). Opens a window briefly for each render.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release
out=web/icons
tmp=$(mktemp -d)
mkdir -p "$out"

render() { # <scale> <name>
  read -r x y side < <(GRUMPY_ICON="$tmp/$2-full.png" GRUMPY_ICON_SCALE="$1" target/release/grumpy-pigs | tail -1)
  x=${x%.*}; y=${y%.*}; side=${side%.*}
  sips -c "$side" "$side" --cropOffset "$y" "$x" "$tmp/$2-full.png" --out "$tmp/$2.png" >/dev/null
}

# "any" icons: the pig fills most of the square.
render 0.36 any
# Maskable: launchers may crop to a circle, so keep the pig inside the central 80%.
render 0.27 maskable

sips -z 512 512 "$tmp/any.png" --out "$out/icon-512.png" >/dev/null
sips -z 192 192 "$tmp/any.png" --out "$out/icon-192.png" >/dev/null
sips -z 512 512 "$tmp/maskable.png" --out "$out/maskable-512.png" >/dev/null
sips -z 180 180 "$tmp/maskable.png" --out "$out/apple-touch-icon.png" >/dev/null
rm -rf "$tmp"
ls -l "$out"
