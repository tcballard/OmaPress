# OmaPress v0.0.1

An early build for getting articles out of a ChatGPT thread and into a publication you own.

Paste or write Markdown, switch to a full-width preview, and add banner artwork above the title. Keep articles locally, generate a website and feeds, and review each publishing destination before sending.

Included in this build:
- Native Qt desktop app and Rust CLI.
- Markdown/Preview toggle, banner import and drag-and-drop, Replace and Remove.
- Static website, RSS, Atom and JSON feeds, with guarded GitHub Pages publishing.
- Experimental X API publishing and separate browser exports.
- Substack browser companion, plus an optional unofficial Gateway integration for drafts and scheduling.
- Local or SSH worker queues for Website/X. Publishing while the laptop is closed requires an always-on worker.

This is the first 0.0.n testing release. We will iterate through 0.0.2, 0.0.3 and onward; v0.1.0 is reserved for a build ready for normal use. XPS/Wayland acceptance, native drag-and-drop, accessibility and real provider workflows are still pending. Browser-based Substack publishing requires its final audience/email review in Substack.

Use the Linux x86_64 tarball and its SHA256SUMS file. Extract it and run `sh install.sh "$HOME/.local"`. Runtime dependencies on Omarchy: qt6-base, qt6-declarative, qt6-wayland, git, github-cli, curl, libsecret, xdg-utils and optionally python for companion registration. Launch `$HOME/.local/bin/omapress-desktop`.

The checked-in Arch recipe still pins an older development snapshot; it is not the v0.0.1 release package. Use this release's tarball for the latest interface. No OPR submission or live Omarchy acceptance is claimed.
