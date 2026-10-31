#!/bin/sh
# Install RustAPI for the current user: binary, launcher entry and icons.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
prefix=${PREFIX:-$HOME/.local}
install -Dm755 "$here/rustapi" "$prefix/bin/rustapi"
install -Dm644 "$here/share/applications/rustapi.desktop" "$prefix/share/applications/rustapi.desktop"
for icon in "$here"/share/icons/hicolor/*/apps/rustapi.png; do
    size=$(basename "$(dirname "$(dirname "$icon")")")
    install -Dm644 "$icon" "$prefix/share/icons/hicolor/$size/apps/rustapi.png"
done
command -v update-desktop-database >/dev/null && update-desktop-database "$prefix/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q "$prefix/share/icons/hicolor" || true
echo "RustAPI installed to $prefix (make sure $prefix/bin is on PATH)"
