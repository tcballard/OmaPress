#!/usr/bin/env sh
set -eu
prefix=${1:-"$HOME/.local"}
base=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$prefix/bin" "$prefix/share/applications" "$prefix/share/icons/hicolor/scalable/apps" "$prefix/share/licenses/omapress"
install -m 755 "$base/bin/omapress" "$base/bin/omapress-desktop" "$prefix/bin/"
install -m 644 "$base/share/omapress/omapress.desktop" "$prefix/share/applications/"
install -m 644 "$base/share/omapress/omapress.svg" "$prefix/share/icons/hicolor/scalable/apps/"
install -m 644 "$base/LICENSE" "$prefix/share/licenses/omapress/LICENSE"
printf 'Installed OmaPress into %s. Ensure %s/bin is on PATH.\n' "$prefix" "$prefix"
