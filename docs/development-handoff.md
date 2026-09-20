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


## Substack Gateway follow-on (2026-09-19)

The user supplied the TypeScript Substack API repository. Its maintained gateway
successor at `8f4dc234b6bc8db04bbfeb407b2ee85bb69d16fe` includes the required draft,
prepublish, schedule and unschedule contracts. Adds an optional Rust client with
native connection/review controls, private session storage, local-image embedding,
account/version-bound durable draft receipts and no blind mutation retries. It
hands the scheduled release to Substack; the website/X worker remains separate.
This supersedes the earlier claim that Substack has only a browser path.

Reproduced: six new gateway integration tests, ten queue integration tests and
eleven distribution tests using actual CLI processes with provider doubles;
Rust fmt, clippy with warnings denied and 29 Rust tests. See
`substack-gateway-inputs.sha256` for inputs. No live gateway/account acceptance ran;
no session credentials were supplied or stored and no article was sent remotely.
Native controls are included in CI dialog smoke. Source contract and deployment
instructions are documented in `substack-gateway.md`. Gateway requests are bounded,
owned by the CLI process and covered by the publication lock through final receipt
writes. The existing Arch recipe remains pinned to the pre-scheduling snapshot.

## Merged-feature packaging revision (2026-09-19)

Arch revision 2 pins merged main `5a2a681598d41a45d6b732cfa6be80045b40671c`.
Downloaded codeload archive SHA256:
`66ce71f0a13f106d82f6885bbc5b1951d063066c27803eb1170f0199613b91f3`.
The reviewed recipe now runs queue and gateway integration suites against that
source, installs optional worker templates and documents, and rewrites the Arch
worker executable to `/usr/bin/pressroom`. It never enables a timer. Portable bundles
also include worker templates and installed documentation. CI now stops on makepkg
failure and emits an Arch package checksum alongside inspection reports.

Local shell syntax, Python compilation and whitespace checks passed. Actual Qt and
Arch build/install verification is delegated to the packaging PR's CI; see that run
for its final result. Earlier evidence above remains historical. Live acceptance is
specified in `docs/live-acceptance.md` and remains unrun without accounts and hosts.

## Final OmaPress naming (2026-09-20)

Tom selected OmaPress as the final name. Executables, desktop identity, Rust crates,
worker templates, portable artifacts, Arch package and current documentation now
use OmaPress/omapress. See `renaming.md` for compatibility and worker upgrade steps.
Earlier entries and checksum files remain historical, including their original paths.
The packaging PR validates the renamed source through engine/native/Arch CI.
Live XPS and provider acceptance remain pending; no OPR submission is made here.


## Editor polish — 2026-09-20

Based on main `48abcf88ff060ad36a816c08dc80976fdd528d95`; desktop input hashes are in `editor-polish-inputs.sha256`.

Completed: mutually exclusive Markdown and Preview views retaining one editor instance; a quieter header, sentence-case actions, explicit Save/Details/More controls, themed selection, narrower navigation, and read-only title/summary/body scrolling together in Preview. Find returns to Markdown. Preview escapes title/summary before adding them to rendered HTML. No publication data or publishing adapters changed.

Reproduced now on Ubuntu 24.04, Qt 6.4.2, software offscreen renderer, scale 1, default OmaPress fallback palette: CMake Release build; actual-app captures at 1440×930 for both modes and 900×930 for Preview; existing `--dialogs-smoke`; `git diff --check`, all exit 0. Captures use an isolated copy of the repository's sanitised fixing-everything fixture, no connected accounts. Screenshot options `--screenshot-preview` and `--screenshot-compact` only apply with `--screenshot PATH`. Images are actual Ubuntu renders of project-owned UI/sample content, supplied for visual review, not Omarchy acceptance.

Known: existing undefined-to-bool warnings from unopened connection dialogs remain, also reproduced before this change. No new layout binding-loop warnings in the final capture. Live Hyprland/Wayland, keyboard/IME, real provider accounts and XPS acceptance not run. CI and Arch packaging results from earlier commits are historical and do not establish verification of this change. The pinned Arch recipe still references the preceding package source; a new package source pin is needed before distributing this UI in that package.


## Banner controls — 2026-09-20

Follow-up to editor polish; inputs in `banner-inputs.sha256`. Above-title artwork area accepts a single local dropped file or file-picker selection, routes through the existing bounded/image-validated media import, and provides Replace/Remove. Changes require Save; removing a reference does not delete shared media. Imported artwork is shown at its original aspect ratio in both modes. A late import cannot attach artwork to a different selected article. Local thumbnail URLs are resolved inside the canonical media directory; HTTPS artwork references remain supported.

Reproduced now: native CMake build and dialog smoke exit 0 on Ubuntu 24.04 / Qt 6.4.2; isolated sample PNG import, save and reload through RPC; actual-app empty-banner and populated Preview captures; diff check exit 0. Capture uses original demonstration artwork created for this test, not a published article card. Existing connection-dialog boolean warnings remain. Initial missing QUrl include caused a compilation failure, fixed before final successful build. Actual OS drag-and-drop, picker, keyboard/IME and XPS/Wayland checks remain not run; no live desktop acceptance claimed. The prior editor-polish hash manifest retains its historical scope.
