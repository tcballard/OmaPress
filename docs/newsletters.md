# Newsletters (development)

OmaPress can prepare an article for your own newsletter service, manage subscribers,
and submit a reviewed send or schedule. The standalone Rust `newsletter-service`
uses PostgreSQL and Amazon SES. Its service name, `/v1` API, `NEWSLETTER_*`
configuration and keyring identity are independent of the desktop product name.
No app rename or document migration is part of this change.

This is a development implementation, not a deployed email service. Capture mode
allows testing without sending email. Production SES/domain and real XPS acceptance
are still required. Mailcheap informed the feature scope; no Mailcheap code was copied.
Original implementation, MIT, under this repository's licence.

## Run a local capture service

Install Docker with Compose, then from the repository:

```sh
cp deploy/newsletter/.env.example deploy/newsletter/.env
chmod 600 deploy/newsletter/.env
# Edit .env: replace the three secrets with independent `openssl rand -hex 32`
# values, and set your test email. Set NEWSLETTER_SENDING_ENABLED=true to
# process the local capture queue; keep NEWSLETTER_TRANSPORT=capture.
docker compose -f deploy/newsletter/compose.yaml up --build -d
```

The database is private to Compose. The API binds host loopback port 8787.
In the app, open **Newsletters**, enter `http://127.0.0.1:8787` and your admin token,
and connect. The token is stored in Secret Service, never in the publication folder.
Create a newsletter with its own sender/reply-to addresses and postal address.

Each newsletter has a public `/subscribe/SLUG` signup form. Confirmation requires
an explicit POST from the confirmation page; merely following its link does not opt
anyone in. Capture mode records confirmation/test/campaign messages in PostgreSQL.
An authenticated `GET /v1/captures` returns the latest 100 captured/queued records
and their confirmation links. This endpoint is absent in SES mode. Capture messages
have **captured** status and have never been sent to a recipient.

To build without Docker, provision PostgreSQL, export the variables in the example
plus `DATABASE_URL`, then run:

```sh
cargo build --locked -p newsletter-service
cargo run --locked -p newsletter-service
```

The service applies versioned, checksum-checked SQLx migrations at startup.
AWS CLI v2 (the image pins 2.37.11) is required only for the SES transport.

## Desktop workflow

1. Connect once, then use **Load saved connection** on future visits.
2. Select a newsletter. Import an existing opted-in list as CSV with an `email`
   column and record its consent source, or use the signup form. Imports preserve
   existing unsubscribes and global bounce/complaint suppressions. A reader can
   rejoin an unsubscribed newsletter through a fresh confirmed signup.
3. Save an article and mark it Ready. Choose **Prepare saved article**. Markdown,
   title and summary become the email; PNG/JPEG cover/body artwork is uploaded to
   public, content-addressed service URLs. HTTPS is required for artwork; loopback
   capture setup supports text-only articles until given an HTTPS origin.
4. Review the email, sender, reply-to address and current audience. Send a test to
   the operator-configured test address and inspect it in a real mail client when
   using SES. The desktop preview is not an inbox rendering test.
5. Confirm the review, then send now or select a local release time and IANA timezone.
   Ambiguous daylight-saving times are rejected by the existing scheduling parser.
6. Refresh campaign status. The server owns the schedule, so the XPS can be off.
   Cancellation stops pending messages; messages already in flight cannot be recalled.

The selected newsletter is the audience. Separate newsletters can represent weekly,
Today in Omarchy and Explaining Omarchy, with independent subscriptions. There is no
paid tier or arbitrary segmentation in this first implementation.

## Delivery and recovery contract

- A newsletter + article UUID identifies one campaign. Repeating draft creation
  returns that campaign; only an unscheduled draft may change. A scheduled,
  cancelled or completed edition is immutable. A genuinely new edition needs its
  own article UUID. Editing an old article never silently resends it.
- Review returns a content revision and a hash of the exact eligible audience.
  Scheduling checks both under a database lock, snapshots recipients, and inserts
  deliveries in one transaction. New subscribers after scheduling are not added;
  unsubscribes and global suppressions are checked again before dispatch.
- Repeating the same schedule is idempotent. The desktop persists the exact
  Send-now timestamp before the HTTP request. After an uncertain request, refresh
  status before preparing another review. Each test also has a stable request ID
  until acknowledged.
- Multiple service replicas serialize dispatch through a PostgreSQL advisory lock.
  A delivery is durably marked dispatching before calling SES. AWS retries are
  disabled. After two minutes, interrupted claims become **unknown**, never pending.
  A matching SES event can reconcile an unknown delivery. No exactly-once delivery
  claim is made: SES does not provide an idempotency key on this call.
- **accepted** means SES accepted the request; **delivered** means the receiving
  mail server accepted it. Neither proves inbox placement or that someone read it.
  Campaign **complete** means no pending/in-flight campaign work remains: inspect
  counts for unknown, bounced, failed and suppressed messages.
- Bounce and complaint events suppress the address across every newsletter. Event
  deduplication and monotonic status updates prevent replay or late delivery events
  from undoing suppression. Campaign delivery pauses above a 2% bounce or 0.08%
  complaint ratio among attempted messages. Paused campaigns can be cancelled;
  automatic resume/retry is intentionally unavailable pending investigation.
- The worker stops sending if its feedback queue cannot be read or processed.
  SIGTERM stops new work and allows the current bounded attempt to finish.

## Deploy for SES

Keep the service behind HTTPS on an origin you control, using the same public URL
for the lifetime of subscription links and artwork. Give it persistent PostgreSQL,
backups, and an always-on process. Compose is a single-host starting point; it does
not provision DNS, TLS, SES or AWS accounts.

1. Verify an SES sending domain in your chosen region; configure DKIM, custom MAIL
   FROM/SPF and DMARC. Obtain production sending access and confirm account quotas.
2. Create an SES configuration set and an SNS event destination for Send, Delivery,
   Bounce, Complaint, Reject and Rendering Failure. Configure the SNS topic policy
   to allow SES from only your AWS account/configuration set.
3. Subscribe a standard SQS queue to that topic, with **raw delivery disabled**.
   Its resource policy must permit `sqs:SendMessage` from `sns.amazonaws.com` only,
   conditioned on the exact topic ARN. Give it a dead-letter queue and alarms.
   Do not grant the service or arbitrary principals SendMessage permissions. The
   service trusts this IAM boundary and validates the SNS envelope's topic ARN;
   it does not expose an SNS webhook or fetch signing certificates.
4. Give the service role `ses:SendEmail` on your sending identity/configuration set,
   and `sqs:ReceiveMessage`, `sqs:DeleteMessage` on that queue. Use the same region.
   Credentials are provided by the AWS CLI standard chain, preferably an IAM role.
5. Set `NEWSLETTER_PUBLIC_URL=https://…`, `NEWSLETTER_TRANSPORT=ses`, configuration
   set, queue URL and topic ARN. Keep sending disabled until setup is checked.
6. Enable sending, use an explicitly controlled test address / SES mailbox simulator,
   and verify real Delivery/Bounce/Complaint events, one-click unsubscribe headers,
   image loading, suppression, cancellation and restart recovery before using a list.

The default worker is deliberately slow (one message per pass with an additional
one-second interval); set `NEWSLETTER_SEND_INTERVAL_MS` for a longer delay if needed.
Rate limits for public signup are persisted: 100 requests per newsletter/hour,
three per address/newsletter/hour. This is a conservative starting point, not a
high-volume acquisition service. Add perimeter abuse protection for a public rollout.

Secrets must be distinct random values, at least 32 characters. Preserve the signing
key in backups: rotating it breaks existing unsubscribe links. Rotate the admin token
by restarting with a new value and reconnecting the app. Never put either in article
sources, URLs, logs or the static website. Redact token-bearing public paths in proxy
access logs. Database backups include subscriber addresses and message content.

## API and CLI

Admin routes require `Authorization: Bearer TOKEN`; no browser-cookie authentication
or permissive CORS is used. Public routes are limited to signup, confirmation,
unsubscribe, images and health. API bodies are bounded to 6 MiB. Campaigns accept
Markdown without raw HTML, remote HTTPS images, and absolute HTTP(S)/mailto links.
HTML is sanitized and emails are capped below 100 KB. CSV import/export and campaign
audiences are currently capped at 10,000 contacts; over-limit operations fail explicitly.

| Endpoint | Operation |
| --- | --- |
| `GET/POST /v1/newsletters` | List/create sender profiles |
| `GET/POST /v1/newsletters/ID/contacts` | Export CSV/import consent-attested CSV |
| `POST /v1/campaigns` | Idempotent create or update unscheduled article draft |
| `GET /v1/campaigns/ID` | Campaign and delivery counts |
| `GET /v1/campaigns/ID/review` | Preview, revision and audience hash |
| `POST /v1/campaigns/ID/test` | Queue test with request ID and reviewed revision |
| `POST /v1/campaigns/ID/schedule` | Freeze reviewed revision/audience at RFC3339 time |
| `POST /v1/campaigns/ID/cancel` | Cancel pending work |
| `POST /v1/assets` | Upload public PNG/JPEG up to 5 MiB |
| `GET /v1/captures` | Inspect development capture transport only |

All app operations are also available through `omapress rpc`, schema 1. Commands:
`newsletter-connect`, `newsletter-list`, `newsletter-create`, `newsletter-contacts`,
`newsletter-import`, `newsletter-prepare`, `newsletter-review`, `newsletter-test`,
`newsletter-schedule`, `newsletter-refresh`, `newsletter-cancel`. Article actions use
`article`, `expected_source_hash`, and `newsletter_id`; send/test additionally bind the
returned `revision`. Schedule requires `audience_hash`, `reviewed:true`, and either
`send_now:true` or `local_time` + `timezone`. Test requires a UUID `request_id`.
The GUI is the reference client; `crates/omapress-core/src/newsletter.rs` defines
request mapping. A headless operator may use `NEWSLETTER_CREDENTIALS_FILE`, an absolute,
owner-only regular JSON file containing `service_url` and `api_token`.

## Backups, upgrades and removal

Back up PostgreSQL and the signing key together, and preserve publication folders
including `.omapress/newsletter.json`. Store backups securely. Take a database backup
before deploying an upgrade; schema rollback requires restoring that backup with the
matching binary, not deleting migration records. Do not point an old worker at a newer
schema. Shut the service down before a restore so it cannot send from two copies.

`docker compose -f deploy/newsletter/compose.yaml down` removes the running services
and preserves the database volume. Do not add `--volumes` unless you intend to erase
subscriber and delivery history. Disabling the service also disables confirmation and
unsubscribe pages: keep it running, with sending disabled, for links in prior emails.

Remaining work before production: real SES/IAM/domain acceptance, real XPS interaction,
backups/restore exercise, and operational monitoring. Open/click tracking, automated
list cleanup, multi-admin accounts, paid subscriptions and a hosted public article
archive are not implemented; the existing publication website remains the archive.
