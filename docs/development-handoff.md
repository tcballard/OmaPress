# Pressroom distribution development handoff

Uses Build Omarchy Apps main `338b36944f07a60ecc7558fb203bba7bb392eda6` (v0.1.0 metadata), including its architecture/lifecycle additions. Applied Design, Desktop, State, Package and Test guidance. Retains the Qt/C++ desktop and independently testable Rust core.

## Ownership

The native UI owns selection and transient review state; the engine owns article validation and destination receipts. Website deployment retains its existing adapter and recovery ledger. The X adapter owns bounded HTTPS calls and secret lookup. The publication lock covers X network requests through final receipt persistence. Draft IDs are committed before the subsequent publish operation. No daemon, autostart or global shortcut is installed.

## Verification

Historical verification remains in `verification.md` unchanged and applies to its original OmaPress sources. Current development commands and results are recorded in the PR; the PR commit identifies the actual tested tree.

- Rust toolchain used locally: 1.98.1; repository pin remains 1.98.0.
- Platform: Linux development container, not a real Omarchy/Hyprland session.
- Native dependency installation attempt failed because the container could not switch apt's service user; no native desktop execution is claimed from that attempt.
- X account/token was unavailable. Opening Substack publishing in the supported browser returned HTTP 502 / connection refused, so its editor could not be inspected. No live articles were published.
- Arch recipe is a development recipe: release source digest remains unfinished. It is not ready for repository submission.

## Compatibility

Working product name: Pressroom. Existing publication schema and `.omapress/` state locations remain intact. No article/recovery migration occurs. Previous last-publication preferences and the old CLI environment override have read fallbacks. GitHub repository name is unchanged. Pacman metadata replaces/conflicts with the old package name; upgrade/uninstall behavior still needs a disposable Arch acceptance run.

## Release blockers

Automatic three-destination publishing remains incomplete: Substack is assisted, and X API is restricted to plain paragraphs pending live acceptance. Native changes require CI smoke and then real Omarchy acceptance. No release, repository rename or App Store submission is part of this development branch.
