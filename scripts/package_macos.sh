#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <version> <arch>" >&2
    exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$1"
ARCH="$2"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] || {
    echo "Invalid release version: $VERSION" >&2
    exit 2
}
BUNDLE="fyle-${VERSION}-macos-${ARCH}"
STAGE="$ROOT/dist/$BUNDLE"
APP="$STAGE/Fyle.app/Contents"

case "$ARCH" in
    x86_64|aarch64) ;;
    *) echo "Unsupported macOS architecture: $ARCH" >&2; exit 2 ;;
esac

test -x "$ROOT/target/release/fyle"
test -x "$ROOT/target/release/fyle-cli"
test -f "$ROOT/assets/fyle.icns"

rm -rf "$STAGE"
mkdir -p "$APP/MacOS" "$APP/Resources" "$STAGE/bin"
install -m 0755 "$ROOT/target/release/fyle" "$APP/MacOS/fyle"
install -m 0644 "$ROOT/assets/fyle.icns" "$APP/Resources/fyle.icns"
install -m 0755 "$ROOT/target/release/fyle-cli" "$STAGE/bin/fyle-cli"
install -m 0644 "$ROOT/LICENSE" "$ROOT/README.md" "$STAGE/"
cat > "$APP/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleDevelopmentRegion</key><string>en</string>
<key>CFBundleExecutable</key><string>fyle</string>
<key>CFBundleIconFile</key><string>fyle.icns</string>
<key>CFBundleIdentifier</key><string>dev.seridian.fyle</string>
<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
<key>CFBundleName</key><string>Fyle</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$VERSION</string>
<key>CFBundleVersion</key><string>$VERSION</string>
<key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
printf 'APPL????' > "$APP/PkgInfo"
tar -C "$ROOT/dist" -czf "$ROOT/dist/${BUNDLE}.tar.gz" "$BUNDLE"
rm -rf "$STAGE"
