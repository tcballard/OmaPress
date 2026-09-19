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
- Arch recipe now pins the complete PR #5 source commit with a measured archive checksum; the final packaging PR adds unsigned Arch build/install/remove CI. It remains a development snapshot, not a stable release.

## Compatibility

Working product name: Pressroom. Existing publication schema and `.omapress/` state locations remain intact. No article/recovery migration occurs. Previous last-publication preferences and the old CLI environment override have read fallbacks. GitHub repository name is unchanged. Pacman metadata replaces/conflicts with the old package name; upgrade/uninstall behavior still needs a disposable Arch acceptance run.

## PR stack

The stack follows #2 foundation → #3 OAuth connections → #4 rich X content → #5 Substack workflow → the pinned-source packaging PR. Each PR targets the previous branch so its diff is reviewable independently. Build Omarchy Plugins was refreshed from `26ee1e7fc57e4089daea57ff0c51e6a10659e401` (v0.4.0). Its design/test guidance explicitly routes external application state outside the hosted shell. No shell-plugin manifest or shell-hosted QML was added to this standalone application.

Current stack adds native OAuth/refresh, provider-schema-checked rich X requests, local artwork uploads, account-bound draft receipts, a restricted browser native-message outbox, explicit Substack confirmations, and companion packaging. The outbox and keyring mutations use a shared application-state lock. Publishing receipts remain publication-owned. Browser sessions remain browser-owned.

## Release blockers

Unattended three-destination publishing is not claimed: Substack final publication remains browser-reviewed. Authenticated X publishing/rendering and Substack draft filling need live acceptance. Foundation, connection, and rich-X branches passed engine and native CI; the final workflow branch must pass the same gates. Real Omarchy, accessibility, and Arch install/upgrade/removal checks remain unrun. No release, repository rename or App Store submission is part of this development branch.

## Scheduling follow-on (2026-09-19)

Adds plain text/Markdown intake, versioned publication-owned queue, explicit
local/SSH execution, conservative missed-job handling, named-zone scheduling,
durable per-destination results and receipt-based reconciliation. The worker owns
one due job per invocation and retains its queue lock through child teardown and
final writes. Killing it leaves an attempt requiring review. The optional user
timer is installed explicitly; no desktop autostart or service is enabled by packaging.

Reproduced in this Linux development container with Rust 1.98.1: `cargo fmt --all
--check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked` (29 tests), `python3 tests/scheduling.py`
(10 tests), `python3 tests/distribution.py` (11 tests), and `python3
tests/deployment.py` (6 tests). Source identities are recorded in
`scheduling-inputs.sha256`; CI validates the actual PR commit, including the extended
native dialog smoke. No local Qt tools are available. SSH/X tests use provider doubles.

Not run: real worker service/SSH host, live X or Substack publication, real Omarchy
clipboard/keyboard/theme acceptance. Substack docs links returned 403/404 from this
environment; no supported write endpoint was verified. No claim of automatic
Substack publishing is made. The optional systemd worker is the deployment route;
no Kamal container deployment was added in this follow-on.

Known product boundaries: website jobs deploy the whole Ready/Published set;
source edits block queued jobs; remote receipts remain worker-owned; failed
unconfirmed jobs require operator reconciliation; the existing Arch source pin
still points to the prior application snapshot. See `scheduling.md` for operational
setup and uninstall/data-preservation instructions. Neither a VPS deployment nor
credentials were provisioned in this session.
