#!/usr/bin/env bash
# Desktop integration requires neither systemd nor OpenRC services.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
DESKTOP_DIR="${DESKTOP_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/applications}"
SYSTEM_INSTALL=false
SET_DEFAULT=false
UNINSTALL=false
for arg in "$@"; do
    case "$arg" in
        --help|-h) echo "Usage: $0 [--system] [--set-default] [--uninstall]"; exit 0 ;;
        --system) SYSTEM_INSTALL=true; INSTALL_DIR=/usr/local/bin; DESKTOP_DIR=/usr/local/share/applications ;;
        --set-default) SET_DEFAULT=true ;;
        --uninstall) UNINSTALL=true ;;
        *) echo "Unknown option: $arg" >&2; exit 2 ;;
    esac
done
if $UNINSTALL; then
    rm -f "$INSTALL_DIR/fyle" "$INSTALL_DIR/fyle-cli" "$DESKTOP_DIR/fyle.desktop"
else
    cd "$SCRIPT_DIR"
    cargo build --release --locked --bins
    MANAGED_FLAGS=()
    if [[ "${SYSTEM_INSTALL:-false}" == true ]]; then MANAGED_FLAGS+=(--system); fi
    python3 scripts/updater.py install --source "$SCRIPT_DIR" --bin-dir "$INSTALL_DIR" --desktop-dir "$DESKTOP_DIR" "${MANAGED_FLAGS[@]}"

fi
if command -v update-desktop-database >/dev/null; then
    update-desktop-database "$DESKTOP_DIR"
fi
if $SET_DEFAULT && ! $UNINSTALL; then
    command -v xdg-mime >/dev/null || { echo 'Install xdg-utils to set the default file manager' >&2; exit 1; }
    xdg-mime default fyle.desktop inode/directory
    echo 'Registered Fyle for directories in the current user session.'
fi
