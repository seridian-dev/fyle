#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICONSET="$(mktemp -d "${TMPDIR:-/tmp}/fyle-iconset.XXXXXX").iconset"
mkdir -p "$ICONSET"
trap 'rm -rf "${ICONSET%.iconset}" "$ICONSET"' EXIT

command -v rsvg-convert >/dev/null || {
    echo "rsvg-convert is required to render assets/fyle.svg" >&2
    exit 1
}
command -v iconutil >/dev/null || {
    echo "iconutil is required to build a macOS .icns file" >&2
    exit 1
}

for size in 16 32 128 256 512; do
    double=$((size * 2))
    rsvg-convert -w "$size" -h "$size" "$ROOT/assets/fyle.svg" \
        -o "$ICONSET/icon_${size}x${size}.png"
    rsvg-convert -w "$double" -h "$double" "$ROOT/assets/fyle.svg" \
        -o "$ICONSET/icon_${size}x${size}@2x.png"
done
iconutil --convert icns --output "$ROOT/assets/fyle.icns" "$ICONSET"
rsvg-convert -w 256 -h 256 "$ROOT/assets/fyle.svg" \
    -o "$ROOT/assets/fyle-256.png"
