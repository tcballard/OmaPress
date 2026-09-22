# Install and first use

[Back to OmaPress](../README.md)

## Install on Omarchy

The v0.0.2 bundle is for Linux x86_64. Install its runtime dependencies:

```bash
sudo pacman -S --needed qt6-base qt6-declarative qt6-wayland git github-cli curl libsecret xdg-utils
```

Close OmaPress before updating, then download, verify and install:

```bash
(
  set -e
  cd "$(mktemp -d)"
  base="https://github.com/tcballard/OmaPress/releases/download/v0.0.2"
  curl -fLO "$base/omapress-0.0.2-linux-x86_64.tar.gz"
  curl -fLO "$base/SHA256SUMS"
  sha256sum -c SHA256SUMS
  tar -xzf omapress-0.0.2-linux-x86_64.tar.gz
  cd omapress-0.0.2-linux-x86_64
  sh install.sh "$HOME/.local"
)
```

Launch **OmaPress** from your application launcher, or run:

```bash
"$HOME/.local/bin/omapress-desktop"
```

Python is also needed if you register the optional [Substack companion](substack-companion.md).

## Start writing

Choose **Create…** for a new publication, or **Open…** for an existing publication folder. Add an article, save it, and switch between Markdown and Preview to review it. Add banner artwork above the title using **Add banner** or drag-and-drop. **Replace** changes it; **Remove** detaches it without deleting the media file. Save to retain changes.

Writing, building and local preview work offline. For publishing, see [destinations and connections](distribution.md) and the [website publishing guide](development.md#publishing). Plain text or Markdown can be pasted through **Add article…**.

## Update or roll back

The bundle installer updates files under `~/.local`. Existing publication folders, settings and recovery history remain in place. To roll back from 0.0.2, reinstall the [0.0.1 bundle](https://github.com/tcballard/OmaPress/releases/tag/v0.0.1); this update makes no publication data-format changes.

If you installed OmaPress through pacman, use package updates and downgrades for that installation. The bundle instructions above are for the separate user-local install.

For older development installations, see [upgrade notes](renaming.md).

## Remove a user-local bundle

Close OmaPress, then remove only the installed application files:

```bash
rm -f "$HOME/.local/bin/omapress" \
      "$HOME/.local/bin/omapress-desktop" \
      "$HOME/.local/bin/omapress-companion" \
      "$HOME/.local/share/applications/omapress.desktop" \
      "$HOME/.local/share/icons/hicolor/scalable/apps/omapress.svg"
rm -rf "$HOME/.local/share/omapress" \
       "$HOME/.local/share/doc/omapress" \
       "$HOME/.local/share/licenses/omapress"
```

This leaves your publication folders, settings and private recovery history intact. A package installation can instead be removed with `sudo pacman -R omapress`.
