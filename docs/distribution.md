# Article distribution — development status

Pressroom keeps one local article and tracks website, X Articles and Substack independently. This is a development increment, not completion of automatic three-platform publishing.

| Destination | Implemented | Remaining live acceptance |
| --- | --- | --- |
| Website | Existing reviewed GitHub Pages deployment, feeds, recheck and rollback | Real publication/domain deployment |
| X Articles | Official draft/publish API for plain paragraph articles; durable draft IDs and uncertain-outcome protection; rich-copy fallback | User OAuth access, account entitlement, rich text and image mapping |
| Substack | Rich body/title/subtitle copy and explicit user-confirmed publication URL | Authenticated browser adapter, image upload, audience/email review and reliable publication verification |

The selected-destinations action currently supports website and the limited X API adapter. It checks X credentials before starting a selected website deployment. Website publishing includes **all** Ready/Published articles, while X applies to the selected article. Completion is tracked independently; there is no cross-platform rollback. Substack remains a separate assisted step and is labelled accordingly.

## X connection

Install `libsecret` and run a Secret Service provider in your desktop session. Obtain a **user** OAuth access token for your own X developer app with the access required by the [Articles API](https://docs.x.com/x-api/articles/introduction). An app-only bearer token is not sufficient. This build does not implement OAuth sign-in or automatic token renewal.

Store it using an interactive prompt, not a command-line argument:

```sh
secret-tool store --label='Pressroom X user OAuth token' application pressroom service x
```

To disconnect:

```sh
secret-tool clear application pressroom service x
```

The engine reads this token only when requested. It never writes it into the publication, logs, arguments or deployment history. The HTTP helper uses private temporary files and bounded curl requests. No account publishing has been performed as part of development verification.

Current API input deliberately permits only plain paragraphs. Images, headings, links and other Markdown structures refuse automatic publishing rather than silently dropping formatting. Use the rich-copy route for those articles until mappings have been accepted against the live API. API source checked 19 September 2026: the official draft and publish endpoints above.

## Receipts and retries

`.omapress/distribution.json` is versioned private state, separate from the existing website ledger. Each receipt binds the immutable article UUID to its content hash, destination, remote draft ID, URL, outcome and timestamp. A changed article displays **changed**, retaining its old receipt. Editing an already prepared X article requires manual reconciliation; automatic replacement/update is not implemented.

Before a potentially mutating X request, the engine persists **unknown**. A crash, timeout, malformed response or API error leaves that state in place. It will not blindly repeat the request. If a draft ID was already returned, it is saved before attempting publication. Check X manually; if published, record its URL. If only an unresolved draft exists, leave the receipt intact for investigation. No reset button silently discards uncertain state.

User-entered URLs are labelled **confirmed by user**, never provider-verified. Substack supports HTTPS custom-domain `/p/` article URLs. URL input is not fetched automatically. Receipt storage never changes the reviewed source hash or enters the generated website.

## Native workflow

Save the article and select **Publish article…**. The screen lists all destination states, website scope, API capability restrictions and assisted export actions. Review automatic destinations, then explicitly publish the reviewed saved version. Source edits invalidate the request. For Substack, paste title, subtitle and rich body, upload images, and review email/audience delivery in Substack before publishing. Record the published URL afterwards.

## Engine protocol

All requests use `pressroom rpc` with schema 1, publication `path` and `args`:

- `distribution-plan`: `article`, `expected_source_hash`.
- `distribution-review`: the same plus `website` and `x` booleans; returns website remote head.
- `distribution-publish`: same selection plus reviewed `expected_remote_head`.
- `x-draft` / `x-publish`: `article`, `expected_source_hash`.
- `distribution-confirm`: the same plus `target` (`x` or `substack`) and HTTPS `url`.

## Remaining implementation

1. Verify X OAuth and richer payload mapping against an authenticated account before removing capability restrictions.
2. Inspect the authenticated Substack editor; implement its draft and publish adapter against observed controls, including explicit email delivery choice and remote receipt recovery.
3. Complete one real article end to end across all three destinations and verify content/images in each.
4. Run real Omarchy launcher/theme/keyboard acceptance, then clean Arch packaging and App Store submission.

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
