# Article distribution — development status

Pressroom keeps one local article and tracks website, X Articles and Substack independently.

| Destination | Implemented | Remaining live acceptance |
| --- | --- | --- |
| Website | Reviewed GitHub Pages deployment, feeds, recheck and rollback | Real publication/domain deployment |
| X Articles | Native OAuth/refresh, rich draft/publish API, cover/body artwork, durable uncertain-outcome protection | Account entitlement and actual published rendering |
| Substack | Private local outbox, browser companion to fill blank drafts, explicit user-confirmed URL | Current editor paste, image upload, autosave and final browser publication |

The combined action publishes the website and/or X article and prepares the
Substack draft when selected. Website publishing includes **all** Ready/Published
articles; X and Substack use the selected article. Each result is retained
independently. There is no cross-platform rollback. **Unattended publication to all
three is not implemented:** Substack audience/email review and its final Publish
action remain in the browser. See [companion setup](substack-companion.md).

## Receipts and retries

`.omapress/distribution.json` is private versioned state. Each receipt binds the
article UUID to its hash, destination, remote ID, URL, outcome and timestamp. X
drafts also bind to the account that created them; switching accounts cannot
publish an old draft. A changed article displays **changed**, retaining its receipt.
Editing an already prepared or published X article requires manual reconciliation;
automatic replacement/update is not implemented.

Before draft creation or publication, X persists **unknown**. A crash, timeout,
malformed response or API error retains it and blocks blind retries. Successful
draft IDs are saved before publication. Check X manually and record the public URL
if published. An unresolved draft should retain its receipt for investigation.

Substack preparation records **awaiting browser**. A paste never marks it published.
Recorded URLs are **confirmed by user**, never provider-verified. Custom-domain
HTTPS `/p/` URLs are supported. URLs are not fetched automatically. Private
receipts never change source hashes or enter generated websites.

## Engine protocol

Use `pressroom rpc` with schema 1, publication `path` and `args`:

- `distribution-plan`: `article`, `expected_source_hash`.
- `distribution-review`: same plus `website`, `x`, `substack` booleans.
- `distribution-publish`: same selection plus reviewed `expected_remote_head`.
- `x-draft`, `x-publish`, `substack-prepare`: `article`, `expected_source_hash`.
- `distribution-confirm`: same plus `target` (`x` or `substack`) and HTTPS `url`.
- `x-connect`: public Native App `client_id`; `x-status` and `x-disconnect`: no args.

The separate native browser protocol is deliberately restricted to prepared outbox
entries. It cannot invoke arbitrary engine commands or read credentials.

## Acceptance and release work

Live account authentication/publishing, Substack editor acceptance, and one real
article across all destinations remain unrun. Real Omarchy launcher/theme/keyboard
and Arch install/upgrade/removal acceptance also remain required. The Arch recipe
still needs a versioned release source and checksum before App Store submission.
No release or App Store submission is claimed by this development stack.

## Native X sign-in (stack layer 1)

Open **Connections**, enter the client ID of your X developer **Native App**
(public client, no client secret), and choose **Connect X**. Register exactly
`http://127.0.0.1:39123/callback` in the developer console. Sign-in requests
`tweet.read tweet.write users.read offline.access media.write`. X account/plan
access to Articles is still required; connecting does not prove that entitlement.

The engine uses PKCE S256, a random state, a loopback-only callback, and a
three-minute deadline. It verifies the account using `/2/users/me` before storing
the token bundle in Secret Service. Refresh happens before an expiring token is
used. Disconnect removes the local credential; revoke application access in X
settings to revoke it at the provider. No password, token, or client secret goes
in a publication. `curl`, `xdg-open`, and `secret-tool` are runtime dependencies.

References: [X OAuth public-client flow](https://docs.x.com/fundamentals/authentication/oauth-2-0/authorization-code).
Live X sign-in and account entitlement checks remain acceptance work on an
Omarchy desktop with the author's developer app.

## Rich X content (stack layer 2)

The API adapter preserves headings 1–3, emphasis, links, quotations, Unicode,
Markdown lists/tables/code, dividers, and standalone local PNG/JPEG images. Cover
art uses `cover_media`. Referenced assets are validated before upload and each
unique asset is uploaded once per draft attempt. Only local imported images up to
5 MiB are accepted; animated/video media and images inside Markdown embeds need
the browser export. Failed media uploads cannot create an Article; failed draft
or publish requests retain the existing conservative unknown-outcome receipt.

The frozen contract in `tests/contracts/x-article-schema.json` comes from
[the provider OpenAPI document](https://api.x.com/2/openapi.json), retrieved
2026-09-19. Its exact lowercase style/entity enums and prohibited extra fields are
checked in the end-to-end provider-double test. This verifies request contracts,
not rendering fidelity or account entitlement on a live X account.
