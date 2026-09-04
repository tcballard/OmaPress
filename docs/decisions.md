# Implementation decisions — 4 September 2026

The supplied specification is the product contract. Tom authorised the full v1 implementation, native Qt styling, MIT or Apache licensing, separate private publication source and public generated output, and delivery through a versioned release and first live publication. MIT is selected. `weekinomarchy.com` is provisional until confirmed; DNS must not be changed based on the tentative spelling. Tom will provide editorial articles and branding later and perform real Omarchy acceptance.

## Publishing contract details

- Source front matter expresses editorial intent (`draft`, `ready`, legacy `published`). Verified local deployment history determines whether the current revision is actually Published. A successful deployment never edits the source.
- Public builds include only ready/published articles; private previews may include drafts and never become deployable artifacts. Future publication dates block a public build. Scheduling is outside v1.
- Review binds to a SHA-256 snapshot of configuration, content, original media, allowed static assets and themes. Builds use this immutable in-memory snapshot. Deployment rebuilds that exact snapshot and verifies the hash before push. Source changes invalidate review.
- The first verified URL, GUID and publication date are retained in an identity ledger. Series route, base URL and slug changes that alter an existing URL are blocked. Corrections update `updated_at` and retain the GUID. Rollback does not erase identity history.
- All filesystem writes use locks and atomic replacement, reject symlink traversal and compare expected source hashes. Locks coordinate OmaPress processes; arbitrary external writers cannot be locked by the app. The deployment snapshot remains immutable even if another program subsequently changes source.
- Unknown push/public-site verification outcomes are journalled before pushing. Recheck discovers the outcome without pushing again. Rollback creates a new commit containing a previous verified output; it never rewrites remote history.
- GitHub authentication stays in `gh`; Git uses `gh auth git-credential` as a helper. No token is read into application state. Git subprocesses disable hooks and interactive prompts.
- Only referenced media and a small static allowlist enter public output. Draft-only media, source filenames, local state and original Markdown never enter the output. Public manifests contain aggregate source hashes and public output hashes.
- Article metadata is YAML schema 1; unknown schemas and keys fail closed. No migrations are needed before schema 2 exists.
- The desktop owns no publication rules: one-shot JSON protocol requests go to the Rust engine. The explicitly started loopback preview child is the only long-lived process and exits with its parent or when preview stops.

## Acceptance boundaries

X clipboard compatibility, Omarchy RSS reader interoperability, accessibility and XPS performance require actual desktop acceptance. They are not inferred from unit tests. The absence of real articles and confirmed domain does not block product development; it does block truthful claims that the real publication has launched.
