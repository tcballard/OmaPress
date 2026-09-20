# Architecture

`omapress-core` owns configuration and article parsing, immutable snapshots, diagnostics, media import, deterministic rendering, feeds, X exports, recovery state and GitHub Pages deployment. `omapress-cli` exposes both ordinary commands and a one-request JSON protocol. No daemon is installed.

The desktop is a small C++ executable with embedded QML. `Bridge` starts bounded asynchronous engine subprocesses and carries versioned JSON over stdin/stdout. It queues state-changing operations, coalesces render/recovery requests, exposes Qt file dialogs and MIME clipboard actions, and reads Omarchy `current/theme/colors.toml` with live theme watching. Rules and file mutations stay in Rust. Editor keystrokes stay on the UI thread and do not wait for builds.

The local preview serves an immutable generated site from memory at a random loopback-only URL. It checks Host headers, refuses traversal and non-GET/HEAD methods, sends no-store/noindex/nosniff headers and a restrictive CSP, and provides no filesystem listing or mutation endpoint. The desktop terminates it; on Linux parent death terminates it too.

## Publication boundary

A snapshot reads only publication.toml, content/, media/, themes/ and static/. It rejects symlinks, special files, invalid paths and excessive input. Generated output and local state do not affect the source hash. Rendering does not alter original articles or images.

Public builds select Ready/Published articles. Private previews can include drafts. Only image assets referenced by public articles enter public output. The optional static allowlist is intentionally small. Theme templates receive presentation values, cannot access the filesystem or run commands, and have a bounded evaluation budget.

## Identities and correction state

Article UUIDs are immutable even before publication. Successful deployments record canonical URLs and first publication dates. Subsequent saves preserve them and set an update date for corrections. Source status is editorial intent; the current deployment's article hashes determine the displayed Published state. A failed deployment cannot set that state.

Schema 1 is the only supported format. Unknown schema versions and fields are rejected without modifying files. Future migrations must be explicit, backed up and expected-hash guarded.

## Runtime budgets

Input: 256 MiB aggregate, 30,000 files, 32 MiB per media file, 4 MiB per article and 64 KiB front matter. Template evaluation: 100,000 fuel units. Subprocess output and execution time are bounded. The standard package excludes shared Qt libraries; no browser engine is bundled.
