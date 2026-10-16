#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "Usage: $0 VERSION x86_64|aarch64" >&2
    exit 2
fi

VERSION="$1"
TARGET_ARCH="$2"
case "$TARGET_ARCH" in
    x86_64) NFPM_ARCH=amd64 ;;
    aarch64) NFPM_ARCH=arm64 ;;
    *) echo "Unsupported Linux release architecture: $TARGET_ARCH" >&2; exit 2 ;;
esac
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] || {
    echo "Invalid release version: $VERSION" >&2
    exit 2
}

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
cargo build --locked --release --bins

DIST="$ROOT/dist"
STAGE="$DIST/stage"
rm -rf "$STAGE"
mkdir -p "$STAGE/usr/bin" "$STAGE/usr/share/applications" "$STAGE/usr/share/doc/fyle" "$STAGE/usr/share/licenses/fyle"
install -m 0755 target/release/fyle "$STAGE/usr/bin/fyle"
install -m 0755 target/release/fyle-cli "$STAGE/usr/bin/fyle-cli"
install -m 0644 fyle.desktop "$STAGE/usr/share/applications/fyle.desktop"
install -m 0644 LICENSE "$STAGE/usr/share/licenses/fyle/LICENSE"
install -m 0644 README.md "$STAGE/usr/share/doc/fyle/README.md"

ARCHIVE_ROOT="$DIST/fyle-${VERSION}-linux-${TARGET_ARCH}"
rm -rf "$ARCHIVE_ROOT"
mkdir -p "$ARCHIVE_ROOT/bin" "$ARCHIVE_ROOT/share/applications"
install -m 0755 target/release/fyle "$ARCHIVE_ROOT/bin/fyle"
install -m 0755 target/release/fyle-cli "$ARCHIVE_ROOT/bin/fyle-cli"
install -m 0644 fyle.desktop "$ARCHIVE_ROOT/share/applications/fyle.desktop"
install -m 0644 LICENSE README.md "$ARCHIVE_ROOT/"
tar -C "$DIST" -czf "$DIST/fyle-${VERSION}-linux-${TARGET_ARCH}.tar.gz" "$(basename "$ARCHIVE_ROOT")"

export VERSION NFPM_ARCH STAGE_DIR="$STAGE"
for format in deb rpm archlinux; do
    nfpm package --config .nfpm.yaml --packager "$format" --target "$DIST"
done
rm -rf "$STAGE" "$ARCHIVE_ROOT"
