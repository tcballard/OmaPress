# OmaPress

**v0.0.1 — early testing.** Development releases use 0.0.n until the app is ready for v0.1.0.

<img alt="Built for Omarchy: App" height="20" src="https://raw.githubusercontent.com/tcballard/omarchy-badges/75975e5b5bf75e7ede3764bcd2950046f7abfe2c/badges/v1/omarchy-app.svg">

Write locally. Publish a website and feeds. Take your words with you.

OmaPress is a native publishing application for Omarchy. It turns Markdown and local images into a static website, full-content RSS, Atom and JSON feeds, and separate article/caption exports for X Articles.

- **Your files:** articles, settings and media remain in a folder you own.
- **Your review:** a publish confirmation is tied to the exact source you reviewed.
- **Your archive:** stable article URLs and feed identities survive corrections.
- **Your recovery:** interrupted deployments can be rechecked and verified versions can be republished.

The editor has a full-width **Markdown / Preview** switch. Preview shows the title, summary and rendered body together; returning to Markdown keeps the same editor and undo history. Add banner artwork above the title using **Add banner** or drag-and-drop. **Replace** changes it; **Remove** detaches it without deleting the media file. Save to retain changes. The banner is visible in both Markdown and Preview. Details and secondary actions are available beside Save.

The desktop uses C++ and Qt 6/QML. The Rust CLI is independently useful. No account or network connection is required for writing, building or local preview. GitHub Pages is the first deployment target; authentication stays with `gh`. The publishing panel tracks website, X and Substack separately. X has browser sign-in, keyring token refresh, and an experimental rich-content API adapter with cover/body image upload. Substack has a local outbox and optional browser companion that fills a blank draft. **Substack still requires its final audience/email review and Publish action in the browser.** See [distribution status and connections](docs/distribution.md) and [Substack companion setup](docs/substack-companion.md).

Existing development install? See the [OmaPress upgrade notes](docs/renaming.md).

## Build and run

On Omarchy/Arch, install `base-devel cmake qt6-base qt6-declarative qt6-wayland git github-cli curl libsecret xdg-utils` and Rust using the pinned toolchain.

```sh
cargo build --release --locked
cmake -S desktop -B build/desktop -DCMAKE_BUILD_TYPE=Release
cmake --build build/desktop --parallel 2
OMAPRESS_CLI="$PWD/target/release/omapress" build/desktop/omapress-desktop
```

Create a publication from the desktop, or use the CLI:

```sh
omapress init ~/Publications/my-publication --name 'My publication' \
  --author 'Your name' --base-url 'https://your-confirmed-domain.example'
omapress inspect ~/Publications/my-publication --json
```

Write articles with schema 1 front matter. The [sample publication](fixtures/fixing-everything/) contains three explicitly labelled sample editions. Use `inspect` to obtain `result.source_hash`, then:

```sh
omapress build ~/Publications/my-publication --expected-source-hash HASH --json
omapress feed-check ~/Publications/my-publication --json
omapress preview ~/Publications/my-publication --json
omapress export-x ~/Publications/my-publication/content/series/article.md --format text
```

The preview prints a private loopback URL and runs only while explicitly open. Add `--drafts` to preview drafts. The desktop automatically closes its preview child on exit.

## Publishing

1. Run `gh auth login` outside OmaPress.
2. Select or create a destination repository and save it in publication settings.
3. Mark reviewed articles Ready, set their dates, then save and run checks.
4. Open the local site preview and inspect it.
5. Choose Publish, review the repository, branch, remote commit and article count, then confirm.
6. A deployment becomes Published only after the remote commit and public files match. An unknown outcome offers Recheck, without another push.

For command-line publishing, `publish-plan` returns the source hash and expected remote head. Both are required by `publish`. `recheck` resolves pending outcomes. `rollback` republishes the output of a verified deployment without altering article sources.

Use separate private source and public output repositories. Keep `.omapress/` backed up securely: it holds recovery copies and deployment history. It never enters the generated site. Configure a custom domain and HTTPS in GitHub Pages only after you have confirmed ownership and DNS. See the [publishing contract](docs/publishing-contract.md).

## Verification and status

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 tests/deployment.py
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software build/desktop/omapress-desktop --smoke
```

Readiness for v0.1.0 requires Tom's real Omarchy acceptance: keyboard and accessibility checks, RSS reader subscription, current X Article paste behaviour, XPS performance, and the real Fixing Everything import/domain launch. Sample fixtures are not the publication's real articles.

[Specification](docs/specification.md) · [Architecture](docs/architecture.md) · [Protocol](docs/protocol.md) · [Acceptance checklist](docs/acceptance.md) · [Security](docs/security.md)

MIT licence. The real Fixing Everything publication is separate from this product repository.

## Upgrading from OmaPress

The app and binaries are now `omapress` / `omapress-desktop`. Open existing publication folders directly; `.omapress/` remains the private recovery/deployment directory so no history migration is required. The previous desktop last-publication setting and `OMAPRESS_CLI` override are read as fallbacks. The GitHub repository remains `tcballard/OmaPress` during development.

For a package installation, uninstall with `sudo pacman -R omapress`; publication folders and private recovery history are user data and must be retained. To roll back, install the previous package and reopen the same folder. New distribution receipts are separate from the old deployment ledger. No live Omarchy version or App Store acceptance is claimed.

### Paste and schedule

Use **Add article…** for plain text or Markdown from ChatGPT, then save and review
it. **Queue…** schedules the saved version for Website and/or X on a local or SSH
worker. An always-on worker can publish while the laptop is closed. Substack can use the optional [Gateway integration](docs/substack-gateway.md)
to create drafts and submit schedules directly, or use the browser handoff. Website jobs deploy all Ready/Published articles together.
See [scheduling and worker setup](docs/scheduling.md) for the source-change rules,
credentials, timer installation, cancellation and current limitations.
