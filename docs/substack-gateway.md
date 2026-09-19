# Optional Substack Gateway integration

Pressroom can prepare and schedule articles through the self-hosted
[jakub-k-slys/substack-gateway-oss](https://github.com/jakub-k-slys/substack-gateway-oss)
project. This is a session-authenticated integration with Substack's internal
endpoints, not an official publishing SDK or a promise of stable provider support.
The native application remains Qt/Rust; the optional gateway runs separately.

## Source verification

Inspected 2026-09-19:

- TypeScript predecessor `jakub-k-slys/substack-api` at
  `dc0f74040d54da87589ea6851b1359445963bd70`: its README directs new integrations
  to the successor. Some old examples mention `createPost`, but that method is
  absent from the inspected `OwnProfile` implementation.
- Gateway successor at `8f4dc234b6bc8db04bbfeb407b2ee85bb69d16fe`:
  `packages/gateway_drafts/src/gateway_drafts/service.py`, its schemas, REST router,
  image resolver and scheduling tests implement draft creation, prepublish checks,
  scheduling and cancellation. These are the contracts used by this adapter.
- REST paths: POST `/api/v1/drafts`, GET `/api/v1/drafts/{id}`, GET
  `/api/v1/drafts/{id}/prepublish`, POST/DELETE `/api/v1/drafts/{id}/schedule`.
- The gateway maps scheduling to Substack's `/drafts/{id}/scheduled_release`.
  Its schedule response reflects a successful POST; it is not an independent
  readback of a later publication. Pressroom records **scheduled**, not published.

## Setup

Run a pinned gateway locally or on your own HTTPS server. For a local setup,
following the upstream Python/uv requirements:

```sh
git clone https://github.com/jakub-k-slys/substack-gateway-oss.git
cd substack-gateway-oss
git checkout 8f4dc234b6bc8db04bbfeb407b2ee85bb69d16fe
uv sync --all-packages --dev
HOST=127.0.0.1 PORT=5001 uv run python -m substack_gateway.main
```

These setup commands follow the inspected upstream tree; gateway installation and
live session acceptance were not executed in this environment. Keep the gateway
running while preparing/scheduling. It does not need to remain running after
Substack accepts the schedule. A remote gateway must use HTTPS; Pressroom allows
plain HTTP only to numeric loopback. Do not point it at a server you do not trust
with your Substack session.

In **Connections → Configure Substack Gateway**, enter its origin, your
`https://publication.substack.com` origin, and the session values documented by
[upstream authentication](https://github.com/jakub-k-slys/substack-gateway-oss/blob/8f4dc234b6bc8db04bbfeb407b2ee85bb69d16fe/docs/authentication.md).
Pressroom stores them in Secret Service; saving a configuration does not claim
that a live connection has been verified. Cookie values are never put in process
arguments, publication files or displayed responses. Curl redirects are disabled.
The gateway receives the session credentials, base64-encoded as its API requires;
base64 is transport encoding, not encryption.

For a headless integration, opt in to
`PRESSROOM_SUBSTACK_GATEWAY_CREDENTIALS_FILE` pointing to an absolute owner-only
regular JSON file with these fields:

```json
{"gateway_url":"http://127.0.0.1:5001","publication_url":"https://example.substack.com","substack_sid":"YOUR_SESSION","connect_sid":""}
```

Use a private parent directory and mode 0600. Do not put this file in a publication
or commit it. To remove a desktop connection, clear the Secret Service entry with
`secret-tool clear application pressroom service substack-gateway`; for an explicit
headless file, remove that credential file separately. Removing credentials does
not cancel an accepted Substack schedule.

## Prepare and schedule

1. Save the article, mark it Ready and resolve Checks.
2. In **Publish article**, choose **Prepare API draft**. Title, summary and Markdown
   go to the gateway. Local PNG/JPEG images are embedded as data URIs for upload.
   Reference-style images and raw HTML use the browser handoff instead. The cover
   appears at the top of the body; no separate social-preview field is configured.
3. Open Substack, inspect the actual rendered draft and its social preview. Select
   the release time/timezone and explicitly choose the post and email audiences in
   Pressroom. Supported values are everyone or paid-only; email-off is not offered
   because that contract has not been verified.
4. Confirm the review and choose **Schedule on Substack**. Pressroom verifies the
   draft has not changed, runs prepublish validation, then submits the release.
   Substack handles its own schedule while your laptop is off.
5. **Cancel Substack schedule** cancels an acknowledged release. It does not delete
   the draft or retract a post already released.

A draft ID is saved before subsequent calls. Repeating a successful preparation
reuses that ID. A repeated identical accepted schedule returns the existing receipt.
After an uncertain mutating request, Pressroom stops; inspect Substack rather than
blindly repeating. Readback interruptions after creation retain the draft ID and
can safely repeat only the read step. Changing connections or article contents
cannot create a second draft over an existing receipt. Session expiry requires
reconnecting; there is no automatic session-cookie refresh.

Website/X timing remains in Queue. Substack's release is handed to the provider
immediately from its own publishing controls; this release does not make a
cross-provider schedule transaction atomic. Immediate Substack publication still
uses the browser; the inspected gateway exposes scheduled release rather than a
separate publish-now endpoint.

## Verification

Six adapter integration tests run the real Rust CLI against a deterministic curl
provider double: draft/schedule/cancel, duplicate prevention, unknown results,
remote edits/prepublish failures, connection binding, image embedding and private
credential handling. They verify this client's gateway contract, not real Substack
acceptance. No live article was created, emailed, published or scheduled here.
