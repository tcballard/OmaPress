# Pressroom distribution development handoff

Uses Build Omarchy Apps v0.2.0 (`ac54d4f`, refreshed during development; skill contents unchanged from architecture commit `338b36944f07a60ecc7558fb203bba7bb392eda6`), including its architecture/lifecycle additions. Applied Design, Desktop, State, Package and Test guidance. Retains the Qt/C++ desktop and independently testable Rust core.

## Ownership

The native UI owns selection and transient review state; stale replies from a previously opened publication are discarded. the engine owns article validation and destination receipts. Website deployment retains its existing adapter and recovery ledger. The X adapter owns bounded HTTPS calls and secret lookup. The publication lock covers X network requests through final receipt persistence. Draft IDs are committed before the subsequent publish operation. No daemon, autostart or global shortcut is installed.

## Verification

Historical verification remains in `verification.md` unchanged and applies to its original OmaPress sources. Current development commands and results are recorded in the PR; the PR commit identifies the actual tested tree.

- Current local evidence: 29 Rust tests, 11 mocked distribution/OAuth/native-companion integration tests, six deployment integration tests, loopback/feed checks, fmt and clippy with warnings denied. Provider doubles do not establish live API acceptance.
- Rust toolchain used locally: 1.98.1; repository pin remains 1.98.0.
- Platform: Linux development container, not a real Omarchy/Hyprland session.
- Native dependency installation attempt failed because the container could not switch apt's service user; no native desktop execution is claimed from that attempt.
- X account/token was unavailable. Opening Substack publishing in the supported browser returned HTTP 502 / connection refused, so its editor could not be inspected. No live articles were published.
- Arch recipe is a development recipe: release source digest remains unfinished. It is not ready for repository submission.

## Compatibility

Working product name: Pressroom. Existing publication schema and `.omapress/` state locations remain intact. No article/recovery migration occurs. Previous last-publication preferences and the old CLI environment override have read fallbacks. GitHub repository name is unchanged. Pacman metadata replaces/conflicts with the old package name; upgrade/uninstall behavior still needs a disposable Arch acceptance run.

## PR stack

The stack follows #2 foundation → #3 OAuth connections → #4 rich X content → the Substack workflow PR. Each PR targets the previous branch so its diff is reviewable independently. Build Omarchy Plugins was refreshed from `26ee1e7fc57e4089daea57ff0c51e6a10659e401` (v0.4.0). Its design/test guidance explicitly routes external application state outside the hosted shell. No shell-plugin manifest or shell-hosted QML was added to this standalone application.

Current stack adds native OAuth/refresh, provider-schema-checked rich X requests, local artwork uploads, account-bound draft receipts, a restricted browser native-message outbox, explicit Substack confirmations, and companion packaging. The outbox and keyring mutations use a shared application-state lock. Publishing receipts remain publication-owned. Browser sessions remain browser-owned.

## Release blockers

Unattended three-destination publishing is not claimed: Substack final publication remains browser-reviewed. Authenticated X publishing/rendering and Substack draft filling need live acceptance. Foundation, connection, and rich-X branches passed engine and native CI; the final workflow branch must pass the same gates. Real Omarchy, accessibility, and Arch install/upgrade/removal checks remain unrun. No release, repository rename or App Store submission is part of this development branch.
