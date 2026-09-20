#!/usr/bin/env sh
set -eu
prefix=${1:-"$HOME/.local"}
base=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$prefix/bin" "$prefix/share/applications" "$prefix/share/icons/hicolor/scalable/apps" "$prefix/share/licenses/omapress"
install -m 755 "$base/bin/omapress" "$base/bin/omapress-desktop" "$base/bin/omapress-companion" "$prefix/bin/"
install -m 644 "$base/share/omapress/omapress.desktop" "$prefix/share/applications/"
install -m 644 "$base/share/omapress/omapress.svg" "$prefix/share/icons/hicolor/scalable/apps/"
install -m 644 "$base/LICENSE" "$prefix/share/licenses/omapress/LICENSE"
mkdir -p "$prefix/share/omapress"
mkdir -p "$prefix/share/doc/omapress"
cp -R "$base/docs/." "$prefix/share/doc/omapress/"
cp -R "$base/share/omapress/worker" "$prefix/share/omapress/"
cp -R "$base/share/omapress/companion" "$prefix/share/omapress/"
install -m 755 "$base/share/omapress/install-companion.py" "$prefix/share/omapress/"
printf 'Installed OmaPress into %s. Ensure %s/bin is on PATH.\n' "$prefix" "$prefix"
