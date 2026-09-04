# Publishing and recovery

A publication has a source folder, a canonical base URL, and an explicit GitHub repository/branch destination. `gh` owns authentication; Git invokes `gh auth git-credential`. OmaPress never reads a token into its JSON state.

1. `publish-plan` validates the source snapshot and public build, checks authentication and reads the remote branch head. The desktop shows the repository, branch, current commit, base URL, source hash and article count.
2. `publish` requires that reviewed source hash and remote head. It rebuilds from an in-memory snapshot, stages only generated output in a dedicated isolated Git checkout, records a pending deployment durably, and pushes using an exact `--force-with-lease` expectation. The new commit descends from the reviewed head; no remote history is rewritten.
3. The public build manifest and each served output file are verified against the cached artifact. The `.nojekyll` build-control file is not expected to be served by Pages. HTTP redirects are restricted to HTTP(S). A network error, mismatched site or interrupted operation retains an Unknown outcome.
4. `recheck` performs no push. If the remote stayed at its original head, the failed push is resolved as not published. If the intended head is present, public verification resumes. If a different writer moved it, the pending state remains unresolved and the user must inspect it.
5. Only a verified result appends to successful deployment history and updates the article identity ledger.

A dirty generated Git checkout is preserved and blocks another publish. Local source changes after review block deployment; changes after the immutable reviewed snapshot is pushed do not change what was deployed. Source files themselves are never rewritten by a deployment.

## Rollback

Rollback selects a locally cached, previously verified output. It verifies every cached hash, checks the currently reviewed source and remote head, then creates a new commit with that earlier output. The old artifact manifest retains its original source identity. The article identity ledger is cumulative, so rollback cannot accidentally free a published URL for reuse.

Keep `.omapress/state.json` and `.omapress/deployments/` in a secure backup alongside the source. A corrupt state file is an error, never grounds to reset publication history. Recovery drafts also live under this private directory. Source Git repositories should ignore it.

## Domains and first setup

Initialisation accepts a configurable URL. Before first publication, set the actual GitHub Pages URL or confirmed custom domain. Pages branch setup is created only if absent; an existing differing Pages source blocks setup instead of being overwritten. Configure custom-domain DNS and HTTPS with GitHub's normal ownership checks. OmaPress verifies the configured canonical site after deployment.

Tom's tentative domain is `weekinomarchy.com`; it must be confirmed before DNS changes or claims of a domain launch. Real articles and branding will be supplied separately. Product fixtures are never imported into the real publication automatically.

## Source repository privacy

The source folder can be committed to a private repository. Generated output belongs in a separate public repository. The first product release does not require making editorial sources public. Publishing never copies Markdown, publication.toml, recovery state or unreferenced draft media into the generated site.
