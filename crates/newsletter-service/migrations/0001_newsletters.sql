CREATE TABLE newsletters (
 id uuid PRIMARY KEY, slug text NOT NULL UNIQUE, name text NOT NULL,
 from_email text NOT NULL, reply_to text NOT NULL, postal_address text NOT NULL,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE contacts (
 id uuid PRIMARY KEY, email text NOT NULL UNIQUE, suppressed boolean NOT NULL DEFAULT false,
 suppression_reason text, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE subscriptions (
 id uuid PRIMARY KEY, newsletter_id uuid NOT NULL REFERENCES newsletters(id),
 contact_id uuid NOT NULL REFERENCES contacts(id),
 status text NOT NULL CHECK(status IN ('pending','active','unsubscribed')),
 consent_source text NOT NULL, consent_at timestamptz,
 confirmation_hash text, confirmation_expires timestamptz,
 updated_at timestamptz NOT NULL DEFAULT now(), UNIQUE(newsletter_id,contact_id)
);
CREATE TABLE campaigns (
 id uuid PRIMARY KEY, newsletter_id uuid NOT NULL REFERENCES newsletters(id),
 external_id uuid NOT NULL, subject text NOT NULL, preview_text text NOT NULL,
 markdown text NOT NULL, revision text NOT NULL,
 status text NOT NULL DEFAULT 'draft' CHECK(status IN ('draft','scheduled','sending','complete','cancelled','paused')),
 scheduled_at timestamptz, created_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE(newsletter_id,external_id)
);
CREATE TABLE deliveries (
 id uuid PRIMARY KEY, dedupe_key text NOT NULL UNIQUE,
 newsletter_id uuid NOT NULL REFERENCES newsletters(id), campaign_id uuid REFERENCES campaigns(id),
 subscription_id uuid REFERENCES subscriptions(id), recipient text NOT NULL,
 kind text NOT NULL CHECK(kind IN ('campaign','confirmation','test')),
 subject text NOT NULL, html text NOT NULL, plain text NOT NULL, unsubscribe_url text,
 state text NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','dispatching','captured','accepted','delivered','failed','unknown','suppressed','cancelled','bounced','complained')),
 due_at timestamptz NOT NULL, started_at timestamptz, provider_id text UNIQUE,
 last_error text, created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX deliveries_due ON deliveries(due_at) WHERE state='pending';
CREATE INDEX deliveries_campaign ON deliveries(campaign_id);
CREATE TABLE provider_events (id text PRIMARY KEY, received_at timestamptz NOT NULL DEFAULT now());
CREATE TABLE rate_limits (key text PRIMARY KEY, window_start timestamptz NOT NULL, count integer NOT NULL);
CREATE TABLE assets (digest text PRIMARY KEY, mime text NOT NULL, bytes bytea NOT NULL, created_at timestamptz NOT NULL DEFAULT now());
