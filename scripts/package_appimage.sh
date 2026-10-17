#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "Usage: $0 VERSION x86_64|aarch64" >&2
    exit 2
fi

VERSION="$1"
TARGET_ARCH="$2"
case "$TARGET_ARCH" in
    x86_64)
        DEPLOY_ARCH=x86_64
        DEPLOY_SHA256=8aea8da0f7f7039d2a2cecb14657d752a222a5e1d3825caeef186c82f751cdd1
        ;;
    aarch64)
        DEPLOY_ARCH=aarch64
        DEPLOY_SHA256=5c1fddf96066891e829831cac0d84424690f3b22846c7f8f1bb9990a5c6c73f4
        ;;
    *) echo "Unsupported AppImage architecture: $TARGET_ARCH" >&2; exit 2 ;;
esac

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
WORK_TMP="${RUNNER_TEMP:-${TMPDIR:-/tmp}}"
APPIMAGE_TOOL="$WORK_TMP/linuxdeploy-${DEPLOY_ARCH}.AppImage"
APPDIR="$WORK_TMP/fyle-AppDir"
rm -rf "$APPDIR" "$WORK_TMP/fyle-appimage-output"
mkdir -p "$APPDIR" "$WORK_TMP/fyle-appimage-output"
curl --fail --location --retry 3 \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${DEPLOY_ARCH}.AppImage" \
    --output "$APPIMAGE_TOOL"
printf '%s  %s\n' "$DEPLOY_SHA256" "$APPIMAGE_TOOL" | sha256sum --check --status
chmod +x "$APPIMAGE_TOOL"

LIB_ARGS=()
for soname in libwayland-client.so.0 libxkbcommon.so.0; do
    library="$(ldconfig -p | awk -v name="$soname" '$1 == name && !found { print $NF; found = 1 }')"
    if [[ -n "$library" ]]; then
        LIB_ARGS+=(--library "$library")
    fi
done

(
    cd "$WORK_TMP/fyle-appimage-output"
    APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGE_TOOL" \
        --appdir "$APPDIR" \
        --executable "$ROOT/target/release/fyle" \
        --executable "$ROOT/target/release/fyle-cli" \
        --desktop-file "$ROOT/fyle.desktop" \
        --icon-file "$ROOT/assets/fyle.svg" \
        "${LIB_ARGS[@]}" \
        --output appimage
)

APPIMAGE_FILE="$(find "$WORK_TMP/fyle-appimage-output" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
[[ -n "$APPIMAGE_FILE" ]] || { echo "linuxdeploy did not produce an AppImage" >&2; exit 1; }
APPIMAGE_OUTPUT="dist/fyle-${VERSION}-${TARGET_ARCH}.AppImage"
install -Dm755 "$APPIMAGE_FILE" "$APPIMAGE_OUTPUT"
APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGE_OUTPUT" --appimage-version >/dev/null
rm -rf "$APPDIR" "$WORK_TMP/fyle-appimage-output" "$APPIMAGE_TOOL"
