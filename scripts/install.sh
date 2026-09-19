#!/usr/bin/env sh
set -eu
prefix=${1:-"$HOME/.local"}
base=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$prefix/bin" "$prefix/share/applications" "$prefix/share/icons/hicolor/scalable/apps" "$prefix/share/licenses/pressroom"
install -m 755 "$base/bin/pressroom" "$base/bin/pressroom-desktop" "$prefix/bin/"
install -m 644 "$base/share/pressroom/pressroom.desktop" "$prefix/share/applications/"
install -m 644 "$base/share/pressroom/pressroom.svg" "$prefix/share/icons/hicolor/scalable/apps/"
install -m 644 "$base/LICENSE" "$prefix/share/licenses/pressroom/LICENSE"
printf 'Installed Pressroom into %s. Ensure %s/bin is on PATH.\n' "$prefix" "$prefix"
