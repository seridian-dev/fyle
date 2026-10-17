#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
    echo "Usage: $0 VERSION x86_64|aarch64" >&2
    exit 2
fi

VERSION="$1"
TARGET_ARCH="$2"
[[ "$TARGET_ARCH" == x86_64 || "$TARGET_ARCH" == aarch64 ]] || {
    echo "Unsupported Flatpak architecture: $TARGET_ARCH" >&2
    exit 2
}

if [[ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]]; then
    exec dbus-run-session -- "$0" "$@"
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
flatpak remote-add --user --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
flatpak install --user --noninteractive --assumeyes flathub \
    org.freedesktop.Platform//25.08 org.freedesktop.Sdk//25.08

rm -rf dist/flatpak-build dist/flatpak-repo
flatpak-builder --user --force-clean --repo=dist/flatpak-repo \
    dist/flatpak-build dev.seridian.Fyle.yml
flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo \
    dist/flatpak-repo "dist/fyle-${VERSION}-linux-${TARGET_ARCH}.flatpak" dev.seridian.Fyle
rm -rf dist/flatpak-build dist/flatpak-repo
