# Native Omarchy publishing specification

Status: Proposed
Product name: OmaPress
Canonical repository: https://github.com/tcballard/OmaPress
First publication: Fixing Everything
Target platform: Omarchy Quattro
Document date: 4 September 2026

## 1. Product statement

OmaPress is a local-first publishing application for Omarchy. It turns Markdown articles and local media into a fast static website, valid RSS feeds and copy-ready versions for X Articles.

The canonical product repository is [`tcballard/OmaPress`](https://github.com/tcballard/OmaPress). Product code, specifications, tests, packaging and release automation live there. The lowercase name `omapress` is reserved for installed binaries, commands and machine-readable identifiers.

It is not a hosted newsletter service. The author owns the source files, generated site and Git history. Publishing should work without a proprietary CMS account. Email delivery can be attached later without changing the publication or its URLs.

Fixing Everything will be the first real publication and the acceptance test for the product.

## 2. The problem

The current workflow produces good copy but leaves the publication scattered across chat output and X Articles. X provides distribution, but it does not document a native RSS feed or act as a portable publication archive.

Ghost would solve the website and RSS parts, but it introduces a hosted CMS before Fixing Everything needs memberships, payments or email delivery.

The narrower problem is:

1. Keep daily and weekly articles in a durable local archive.
2. Review and publish them from a native Omarchy application.
3. Generate a real website and RSS automatically.
4. Export clean article copy and captions for X.
5. Preserve the option to add email later.

## 3. Product principles

- Local files are the source of truth.
- Review always comes before publication.
- Publishing is deterministic and recoverable.
- RSS is a first-class output, not an integration added later.
- The public website works without JavaScript.
- X is a distribution target, not the canonical archive.
- A failed deployment never marks an article as published.
- The application stays useful without an account or network connection.

## 4. Intended user

The first user is Tom Ballard publishing Fixing Everything from Omarchy.

The broader user is a technical writer, open-source maintainer or small independent publication that wants a simple native workflow without operating a CMS.

This is a single-author product in version 1. Multiple authors, editorial permissions and hosted collaboration are later concerns.

## 5. Core workflow

### First-time setup

1. Open OmaPress.
2. Create a publication or open an existing publication folder.
3. Enter the publication name, description, canonical domain and author.
4. Choose a publishing target. Version 1 supports GitHub Pages.
5. Authenticate using the existing `gh` login.
6. Select or create the destination repository.
7. Choose a theme and preview the empty publication.
8. Run a preflight check, then publish the initial site.

### Daily publishing

1. Create an article or paste a generated draft.
2. Select the series, such as Today in Omarchy.
3. Set the editorial title, date, summary and header image.
4. Edit in Markdown with a rendered preview beside it.
5. Run preflight checks.
6. Preview the exact generated site locally.
7. Publish the website and feeds.
8. Copy the X Article body and X caption using separate buttons.
9. Paste them into X and publish manually.
10. Optionally record the final X URL against the article.

The app must never publish directly to X in version 1. X remains a deliberate manual step.

## 6. Information architecture

The main window has four areas.

| Area | Purpose |
| --- | --- |
| Publication rail | Switch publication, open settings, view deployment status |
| Article list | Filter by draft, ready, published, series and date |
| Editor | Edit Markdown, metadata and media |
| Preview and checks | Render the article, show feed output and block unsafe publication |

The default article list groups entries under Drafts, Ready and Published. Search covers titles, summaries, body text and tags.

The editor uses a distraction-free writing surface similar to Omawrite, but adds publication metadata and preflight status. It must support standard keyboard editing, undo, find, headings, links, lists, inline code, fenced code, images and paste-as-plain-text.

## 7. Publication model

Each publication is an ordinary directory that can be committed to Git.

```text
fixing-everything/
  publication.toml
  content/
    today-in-omarchy/
      2026-09-04-omarchy-learns-to-handle-its-own-growth.md
    unofficial-week-in-omarchy/
  media/
    2026/
      09/
  themes/
    fixing-everything/
  static/
    favicon.svg
  output/
  .omapress/
    state.json
    preview/
```

`output/` and `.omapress/preview/` are generated. Article source and original media are never modified during a build.

### Publication configuration

```toml
schema = 1
name = "Fixing Everything"
tagline = "We can fix everything"
description = "Independent reporting from the Omarchy ecosystem"
base_url = "https://fixingeverything.example"
language = "en-GB"
timezone = "Europe/London"
author = "Tom Ballard"

[feeds]
full_content = true
limit = 50

[deploy]
provider = "github-pages"
repository = "tcballard/fixing-everything"
branch = "gh-pages"
```

### Article front matter

```yaml
---
schema: 1
id: 018fc5c0-2f54-7ea8-aeb2-75e566f38345
title: Omarchy learns to handle its own growth
series: today-in-omarchy
status: draft
published_at: 2026-09-04T21:00:00+01:00
summary: The marketplace shipped 253 commits while Apple Silicon packaging moved upstream.
slug: omarchy-learns-to-handle-its-own-growth
header_image: /media/2026/09/marketplace-growth.png
x_caption: |
  I counted the marketplace twice today.
source_window:
  starts_at: 2026-09-03T21:00:00+01:00
  ends_at: 2026-09-04T21:00:00+01:00
---
```

The article ID is immutable. The slug becomes immutable after first publication so RSS readers do not receive duplicates when a title changes.

## 8. Website output

Every build produces:

- A publication homepage.
- One permanent page per article.
- Series archive pages.
- Monthly and yearly archive pages.
- An about page.
- A combined RSS 2.0 feed at `/rss.xml`.
- A combined Atom feed at `/atom.xml`.
- A JSON Feed at `/feed.json`.
- Separate RSS feeds for each series.
- `sitemap.xml`, `robots.txt`, favicons and social metadata.
- A machine-readable build manifest containing source hashes and output hashes.

The initial series routes are:

```text
/today-in-omarchy/
/today-in-omarchy/rss.xml
/the-unofficial-week-in-omarchy/
/the-unofficial-week-in-omarchy/rss.xml
```

Feeds contain full article content, stable GUIDs, canonical URLs, publication dates, update dates and image enclosures where appropriate. A feed validation failure blocks deployment.

## 9. X export

The X export view provides two independent copy actions:

- Copy article
- Copy caption

Copy article places both HTML and plain text on the clipboard. The HTML version preserves supported headings, emphasis, lists, links and code when pasted into the X Article editor. The plain-text fallback keeps readable spacing if rich paste fails.

The export strips publication-only elements such as the site navigation and RSS prompt. It includes the masthead, edition label, editorial title and date in the configured order.

Before copying, OmaPress shows unsupported constructs and the resulting X version. It never silently removes content.

## 10. Native architecture

OmaPress is a standalone desktop application. It is not loaded inside the long-running Omarchy shell.

This keeps publishing failures away from the bar, lock screen and other session-critical services. It also gives the editor a normal resizable window, file dialogs and desktop lifecycle.

### Components

| Component | Technology | Responsibility |
| --- | --- | --- |
| `omapress-desktop` | Qt 6 and QML | Native interface, editor, preview, clipboard, file selection |
| `omapress` | Rust CLI | Parse, validate, build, preview and deploy |
| Theme bundle | HTML, CSS and templates | Static public presentation |
| Publication repository | Markdown, TOML and media | User-owned source of truth |

The desktop app invokes the CLI as bounded subprocesses and exchanges versioned JSON over standard input and output. There is no resident daemon in version 1.

The CLI remains independently useful for terminal workflows, agents and CI.

### Product repository layout

The canonical repository starts with this structure:

```text
OmaPress/
  AGENTS.md
  LICENSE
  README.md
  Cargo.toml
  crates/
    omapress-core/
    omapress-cli/
  desktop/
    CMakeLists.txt
    src/
    qml/
    resources/
  themes/
    default/
    fixing-everything/
  schemas/
    publication.schema.json
    article.schema.json
    protocol.schema.json
  fixtures/
    fixing-everything/
  packaging/
    arch/
  scripts/
  tests/
  docs/
    specification.md
    architecture.md
    publishing-contract.md
    security.md
```

The Rust workspace owns parsing, rendering, validation and deployment. The Qt application consumes the versioned CLI protocol and does not duplicate publication rules.

The `fixtures/fixing-everything/` publication is a small sanitised corpus used for deterministic builds, feed validation and clipboard-export tests. The real Fixing Everything publication remains in its own repository so product releases cannot accidentally publish editorial drafts.

### Command contract

```text
omapress init PATH
omapress inspect PATH --json
omapress validate PATH --json
omapress build PATH --output PATH --json
omapress preview PATH --bind 127.0.0.1 --port 0 --json
omapress publish PATH --expected-source-hash HASH --json
omapress export-x ARTICLE --format html
omapress export-x ARTICLE --format text
omapress feed-check PATH --json
```

Every mutating command accepts an expected source hash. If the files changed after review, the command stops instead of deploying a different tree.

## 11. Publishing and recovery

GitHub Pages is the only version 1 deployment target.

The publisher builds into a temporary directory, validates every output, and creates a deployment manifest. It then updates an isolated `gh-pages` worktree and pushes with an expected remote head.

Publication succeeds only after the remote commit and public site can be verified. If verification times out, the app reports an unknown state and offers a safe recheck. It does not push the same deployment blindly.

Each successful deployment records:

- Source commit and tree hash.
- Generated output hash.
- Remote deployment commit.
- Article IDs included.
- Feed hashes.
- Publication timestamp.

Rollback republishes a previous verified deployment. It never rewrites article source.

## 12. Safety requirements

- Markdown is rendered with raw HTML disabled by default.
- Links allow `https`, `http` and `mailto` only.
- Generated paths cannot escape the publication directory.
- Media filenames are normalised and content is copied, never executed.
- Templates cannot invoke shell commands or access arbitrary files.
- Preview binds to `127.0.0.1` on an automatically selected port.
- Git operations show the exact repository, branch and commit before the first publish.
- Authentication stays with `gh`; OmaPress stores no GitHub token.
- Publishing refuses dirty generated worktrees, unexpected remote movement and invalid feed output.
- External images generate a warning. The preferred path is to import them into publication media.
- Secrets, drafts and local state are excluded from generated output by construction.

## 13. Offline behaviour

Writing, editing, media management, validation and local preview work offline.

Publishing requires a network connection. A failed publish leaves the article in Ready state and preserves the reviewed source hash. Reconnecting allows the author to retry after the app confirms that the source and remote branch have not changed.

## 14. Version 1 acceptance criteria

The first release is acceptable when Tom can complete this workflow on a real Omarchy machine:

1. Create the Fixing Everything publication.
2. Import the existing daily and weekly articles.
3. Add a new Today in Omarchy draft.
4. Edit and preview it without network access.
5. Catch a missing editorial title and malformed source link during preflight.
6. Publish the site to GitHub Pages.
7. Subscribe to the combined and daily RSS feeds in the native Omarchy RSS reader.
8. Copy the X article and caption independently.
9. Republish a correction without creating a duplicate RSS item.
10. Roll the public site back to the previous verified deployment.

## 15. Performance targets

- Cold launch to editable article list in under 500 ms on the 2024 XPS 14.
- Keystrokes remain responsive while validation or builds run.
- Incremental preview of one changed article in under 150 ms at the 95th percentile.
- Full build of 1,000 text articles in under two seconds, excluding image processing.
- Application package below 10 MB, excluding shared Qt libraries already present on Omarchy.
- No network request during launch or editing.

These are release targets and require measurement on Omarchy hardware. They are not assumed from synthetic tests.

## 16. Test plan

### Engine

- Front matter schema, migrations and failure cases.
- Markdown and URL sanitisation.
- Stable slugs, GUIDs and canonical URLs.
- RSS 2.0, Atom and JSON Feed validation.
- Deterministic builds from identical source trees.
- Incremental builds and deleted articles.
- Concurrent source changes during build and publish.
- Remote branch movement and interrupted pushes.
- Rollback and unknown deployment states.

### Desktop

- Keyboard-only authoring and publishing.
- Clipboard HTML and plain-text fallbacks.
- Unsaved changes, crash recovery and reopen.
- Long articles, code blocks, large images and invalid metadata.
- Offline startup and failed deployment recovery.
- Screen reader names, focus order, contrast and text scaling.

### End to end

- GitHub Pages deployment from a clean account.
- Custom domain and HTTPS verification.
- Feed subscription and refresh in the Omarchy RSS reader.
- Paste into the current X Article editor.
- Corrected article retains its feed identity.

## 17. Explicit non-goals for version 1

- Subscriber accounts or mailing-list storage.
- Sending email.
- Paid subscriptions, memberships or payments.
- Team roles, comments or collaborative editing.
- Hosted analytics or tracking pixels.
- Direct publishing to X.
- Mobile authoring.
- A general website builder.
- Arbitrary themes downloaded and executed from the internet.
- A permanently running local web server.

## 18. Later extensions

### Email delivery

Add an adapter that sends published posts through a specialist provider. The RSS feed remains the source. OmaPress should not implement mail reputation, bounce handling or unsubscribe compliance itself.

### Scheduled publication

Add a repository workflow that publishes approved, future-dated posts when their time arrives. Local system timers are insufficient because the author's machine may be asleep.

### Agent workflow

Expose a constrained local API for creating drafts, adding sources and running preflight. Agents may prepare content but cannot cross the publish confirmation boundary.

### Multiple publications

Allow one application installation to manage several independent publication directories without mixing their themes, credentials or deployment histories.

### Alternative deployment targets

Add Cloudflare Pages, S3-compatible storage and ordinary SSH deployment behind the same staged deployment contract.

## 19. Proposed implementation stack

Build the product as five reviewable slices.

| Slice | Deliverable |
| --- | --- |
| 1. Publication core | Schema, article parser, deterministic static build, combined RSS |
| 2. Native library | Qt application, publication opening, article list, Markdown editing, recovery |
| 3. Preview and checks | Rendered preview, feed validation, source checks, media handling |
| 4. Publishing | GitHub Pages setup, guarded deployment, verification and rollback |
| 5. Distribution | Per-series feeds, X rich-copy export, Fixing Everything import and real-machine acceptance |

Each slice should be usable from the CLI before its desktop controls are considered complete.

## 20. Open decisions

- Final Fixing Everything domain.
- Whether the source repository remains private while generated output is published separately.
- Whether the initial theme follows the eventual Taha masthead or ships with a temporary typographic treatment.

None of these decisions blocks the publication engine or RSS work.

## 21. Recommended first milestone

The first milestone should be implemented in [`tcballard/OmaPress`](https://github.com/tcballard/OmaPress) and stop at a command-line proof:

1. Initialise a Fixing Everything publication.
2. Import the two existing daily editions and the weekly format.
3. Build the homepage, article pages and three RSS feeds.
4. Validate them locally.
5. Serve the generated site on localhost.
6. Subscribe to its daily feed from the native Omarchy RSS reader.

That proves the central idea before any time is spent polishing an editor.

## 22. Reference points

- [OmaPress repository](https://github.com/tcballard/OmaPress)
- [Omarchy repository and manual](https://github.com/omacom/omarchy)
- [Omarchy Quattro release](https://github.com/omacom/omarchy/releases/tag/v4.0.0)
- [DHH on the Qt and C++ Omarchy applications](https://x.com/dhh/status/2084701529957621890)
- [X Articles documentation](https://help.x.com/en/using-x/articles)
- [Ghost automatic RSS behaviour](https://ghost.org/help/where-can-i-find-my-rss-feed/)
