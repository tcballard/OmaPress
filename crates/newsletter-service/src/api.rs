use crate::{App, Error, Result, digest, mail};
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;
use sqlx::{FromRow, Row};
use uuid::Uuid;

#[derive(Serialize, Deserialize, FromRow)]
pub struct Newsletter {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub from_email: String,
    pub reply_to: String,
    pub postal_address: String,
}
#[derive(Serialize, Deserialize, FromRow)]
pub struct Campaign {
    pub id: Uuid,
    pub newsletter_id: Uuid,
    pub external_id: Uuid,
    pub subject: String,
    pub preview_text: String,
    pub markdown: String,
    pub revision: String,
    pub status: String,
    pub scheduled_at: Option<DateTime<Utc>>,
}

pub fn router(app: App) -> Router {
    let private = Router::new()
        .route(
            "/newsletters",
            get(list_newsletters).post(create_newsletter),
        )
        .route(
            "/newsletters/{id}/contacts",
            get(contacts).post(import_contacts),
        )
        .route("/campaigns", post(create_campaign))
        .route("/campaigns/{id}", get(campaign_status))
        .route("/campaigns/{id}/review", get(review))
        .route("/campaigns/{id}/schedule", post(schedule))
        .route("/campaigns/{id}/cancel", post(cancel))
        .route("/campaigns/{id}/test", post(test_send))
        .route("/assets", post(upload_asset))
        .route("/captures", get(captures))
        .route_layer(middleware::from_fn_with_state(app.clone(), authorize));
    Router::new()
        .nest("/v1", private)
        .route(
            "/health",
            get(|| async { Json(json!({"status":"ok","schema":1})) }),
        )
        .route("/subscribe/{slug}", get(signup_form).post(signup))
        .route("/confirm/{token}", get(confirm_form).post(confirm))
        .route(
            "/unsubscribe/{token}",
            get(unsubscribe_form).post(unsubscribe),
        )
        .route("/assets/{digest}", get(asset))
        .layer(DefaultBodyLimit::max(6 * 1024 * 1024))
        .layer(middleware::from_fn(headers))
        .with_state(app)
}
async fn headers(req: Request<Body>, next: Next) -> Response {
    let mut r = next.run(req).await;
    for (k, v) in [
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("cache-control", "no-store"),
        (
            "content-security-policy",
            "default-src 'none'; img-src https:; style-src 'unsafe-inline'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'",
        ),
    ] {
        r.headers_mut().insert(k, v.parse().unwrap());
    }
    r
}
async fn authorize(State(app): State<App>, req: Request<Body>, next: Next) -> Response {
    let supplied = req
        .headers()
        .get("authorization")
        .and_then(|s| s.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or("");
    // Constant-time comparison, including tokens of different lengths.
    let mut mac = Hmac::<Sha256>::new_from_slice(app.config.signing_key.as_bytes()).unwrap();
    mac.update(app.config.admin_token.as_bytes());
    let expected = mac.finalize().into_bytes();
    let mut mac = Hmac::<Sha256>::new_from_slice(app.config.signing_key.as_bytes()).unwrap();
    mac.update(supplied.as_bytes());
    if mac.verify_slice(&expected).is_err() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":"Authentication required"})),
        )
            .into_response();
    }
    next.run(req).await
}
pub async fn newsletter(app: &App, id: Uuid) -> Result<Newsletter> {
    sqlx::query_as(
        "SELECT id,slug,name,from_email,reply_to,postal_address FROM newsletters WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(&app.db)
    .await?
    .ok_or(Error(StatusCode::NOT_FOUND, "Newsletter not found".into()))
}
pub async fn campaign(app: &App, id: Uuid) -> Result<Campaign> {
    sqlx::query_as("SELECT id,newsletter_id,external_id,subject,preview_text,markdown,revision,status,scheduled_at FROM campaigns WHERE id=$1").bind(id).fetch_optional(&app.db).await?.ok_or(Error(StatusCode::NOT_FOUND,"Campaign not found".into()))
}
async fn list_newsletters(State(app): State<App>) -> Result<Json<Value>> {
    let rows: Vec<Newsletter> = sqlx::query_as(
        "SELECT id,slug,name,from_email,reply_to,postal_address FROM newsletters ORDER BY name",
    )
    .fetch_all(&app.db)
    .await?;
    Ok(Json(
        json!({"newsletters":rows,"transport":app.config.transport,"sending_enabled":app.config.sending_enabled,"test_recipient":app.config.test_recipient}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewNewsletter {
    slug: String,
    name: String,
    from_email: String,
    reply_to: String,
    postal_address: String,
}
async fn create_newsletter(
    State(app): State<App>,
    Json(p): Json<NewNewsletter>,
) -> Result<Json<Value>> {
    if p.slug.is_empty()
        || p.slug.len() > 80
        || !p
            .slug
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        || p.name.trim().is_empty()
        || p.name.len() > 150
        || p.postal_address.trim().is_empty()
        || p.postal_address.len() > 1000
    {
        return Err(Error::bad(
            "Enter a lowercase slug, name and postal address",
        ));
    }
    let from = mail::email(&p.from_email)?;
    let reply = mail::email(&p.reply_to)?;
    // Slug is a natural idempotency key. Never overwrite sender settings on retry.
    sqlx::query("INSERT INTO newsletters(id,slug,name,from_email,reply_to,postal_address) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(slug) DO NOTHING")
        .bind(Uuid::new_v4()).bind(&p.slug).bind(&p.name).bind(&from).bind(&reply).bind(&p.postal_address).execute(&app.db).await?;
    let n: Newsletter = sqlx::query_as(
        "SELECT id,slug,name,from_email,reply_to,postal_address FROM newsletters WHERE slug=$1",
    )
    .bind(&p.slug)
    .fetch_one(&app.db)
    .await?;
    if n.name != p.name
        || n.from_email != from
        || n.reply_to != reply
        || n.postal_address != p.postal_address
    {
        return Err(Error::conflict(
            "This slug already has different sender settings",
        ));
    }
    Ok(Json(
        json!({"newsletter":n,"signup_url":format!("{}/subscribe/{}",app.config.public_url,n.slug)}),
    ))
}
async fn limit(app: &App, key: &str, max: i32) -> Result<()> {
    let count:i32=sqlx::query_scalar("INSERT INTO rate_limits(key,window_start,count) VALUES($1,now(),1) ON CONFLICT(key) DO UPDATE SET count=CASE WHEN rate_limits.window_start < now()-interval '1 hour' THEN 1 ELSE rate_limits.count+1 END, window_start=CASE WHEN rate_limits.window_start < now()-interval '1 hour' THEN now() ELSE rate_limits.window_start END RETURNING count").bind(key).fetch_one(&app.db).await?;
    if count > max {
        return Err(Error(
            StatusCode::TOO_MANY_REQUESTS,
            "Please try again later".into(),
        ));
    }
    Ok(())
}
fn form(title: &str, button: &str, extra: &str) -> Html<String> {
    Html(format!(
        "<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width\"><title>{}</title><main><h1>{}</h1><form method=post>{extra}<button type=submit>{}</button></form></main>",
        mail::esc(title),
        mail::esc(title),
        mail::esc(button)
    ))
}
async fn signup_form(State(app): State<App>, Path(slug): Path<String>) -> Result<Html<String>> {
    let name: Option<String> = sqlx::query_scalar("SELECT name FROM newsletters WHERE slug=$1")
        .bind(slug)
        .fetch_optional(&app.db)
        .await?;
    Ok(form(
        &format!(
            "Subscribe to {}",
            name.ok_or(Error(StatusCode::NOT_FOUND, "Newsletter not found".into()))?
        ),
        "Send confirmation",
        "<label>Email <input type=email name=email required maxlength=254></label><p>We’ll email a confirmation link. You can unsubscribe at any time.</p>",
    ))
}
#[derive(Deserialize)]
struct Signup {
    email: String,
}
async fn signup(
    State(app): State<App>,
    Path(slug): Path<String>,
    axum::Form(p): axum::Form<Signup>,
) -> Result<Html<String>> {
    let email = mail::email(&p.email)?;
    let n: Newsletter = sqlx::query_as(
        "SELECT id,slug,name,from_email,reply_to,postal_address FROM newsletters WHERE slug=$1",
    )
    .bind(&slug)
    .fetch_optional(&app.db)
    .await?
    .ok_or(Error(StatusCode::NOT_FOUND, "Newsletter not found".into()))?;
    limit(&app, &format!("signup:{}", n.id), 100).await?;
    limit(
        &app,
        &format!("signup:{}:{}", n.id, digest(email.as_bytes())),
        3,
    )
    .await?;
    let mut tx = app.db.begin().await?;
    sqlx::query("SELECT id FROM newsletters WHERE id=$1 FOR UPDATE")
        .bind(n.id)
        .execute(&mut *tx)
        .await?;
    let contact:Uuid=sqlx::query_scalar("INSERT INTO contacts(id,email) VALUES($1,$2) ON CONFLICT(email) DO UPDATE SET email=EXCLUDED.email RETURNING id").bind(Uuid::new_v4()).bind(&email).fetch_one(&mut *tx).await?;
    let suppressed: bool = sqlx::query_scalar("SELECT suppressed FROM contacts WHERE id=$1")
        .bind(contact)
        .fetch_one(&mut *tx)
        .await?;
    if !suppressed {
        let raw = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let sub:Option<Uuid>=sqlx::query_scalar("INSERT INTO subscriptions(id,newsletter_id,contact_id,status,consent_source,confirmation_hash,confirmation_expires) VALUES($1,$2,$3,'pending','public signup',$4,now()+interval '24 hours') ON CONFLICT(newsletter_id,contact_id) DO UPDATE SET confirmation_hash=EXCLUDED.confirmation_hash,confirmation_expires=EXCLUDED.confirmation_expires,updated_at=now() WHERE subscriptions.status!='active' RETURNING id")
            .bind(Uuid::new_v4()).bind(n.id).bind(contact).bind(digest(raw.as_bytes())).fetch_optional(&mut *tx).await?;
        if let Some(sub) = sub {
            let link = format!("{}/confirm/{raw}", app.config.public_url);
            let subject = format!("Confirm your subscription to {}", n.name);
            let html = format!(
                "<p>Confirm your subscription to {}.</p><p><a href=\"{}\">Confirm subscription</a></p><p>If you did not request this, ignore this email.</p>",
                mail::esc(&n.name),
                mail::esc(&link)
            );
            sqlx::query("INSERT INTO deliveries(id,dedupe_key,newsletter_id,subscription_id,recipient,kind,subject,html,plain,due_at) VALUES($1,$2,$3,$4,$5,'confirmation',$6,$7,$8,now())")
                .bind(Uuid::new_v4()).bind(format!("confirm:{raw}")).bind(n.id).bind(sub).bind(email).bind(subject).bind(html).bind(format!("Confirm subscription: {link}")).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(Html(
        "Check your inbox for a confirmation link if this address is eligible.".into(),
    ))
}
async fn confirm_form(Path(_): Path<String>) -> Html<String> {
    form(
        "Confirm newsletter subscription",
        "Confirm subscription",
        "",
    )
}
async fn confirm(State(app): State<App>, Path(token): Path<String>) -> Result<Html<String>> {
    if token.len() != 64 {
        return Err(Error::bad("Invalid confirmation link"));
    }
    let n: Option<Uuid> =
        sqlx::query_scalar("SELECT newsletter_id FROM subscriptions WHERE confirmation_hash=$1")
            .bind(digest(token.as_bytes()))
            .fetch_optional(&app.db)
            .await?;
    let mut tx = app.db.begin().await?;
    sqlx::query("SELECT id FROM newsletters WHERE id=$1 FOR UPDATE")
        .bind(n.ok_or(Error::bad("Confirmation expired or already used"))?)
        .execute(&mut *tx)
        .await?;
    let result=sqlx::query("UPDATE subscriptions SET status='active',consent_at=now(),consent_source='confirmed public signup',confirmation_hash=NULL,confirmation_expires=NULL,updated_at=now() WHERE confirmation_hash=$1 AND confirmation_expires>now() AND NOT EXISTS(SELECT 1 FROM contacts WHERE contacts.id=subscriptions.contact_id AND suppressed)").bind(digest(token.as_bytes())).execute(&mut *tx).await?;
    tx.commit().await?;
    if result.rows_affected() == 0 {
        return Err(Error::bad("Confirmation expired or address unavailable"));
    }
    Ok(Html(
        "Subscription confirmed. You can unsubscribe from any newsletter email.".into(),
    ))
}
async fn unsubscribe_form(Path(_): Path<String>) -> Html<String> {
    form("Unsubscribe from this newsletter", "Unsubscribe", "")
}
async fn unsubscribe(State(app): State<App>, Path(token): Path<String>) -> Result<Html<String>> {
    let id = mail::verify_token(&app.config.signing_key, &token)?;
    let mut tx = app.db.begin().await?;
    sqlx::query("SELECT n.id FROM newsletters n JOIN subscriptions s ON s.newsletter_id=n.id WHERE s.id=$1 FOR UPDATE OF n").bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE subscriptions SET status='unsubscribed',confirmation_hash=NULL,confirmation_expires=NULL,updated_at=now() WHERE id=$1").bind(id).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE deliveries SET state='suppressed' WHERE subscription_id=$1 AND state='pending'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Html(
        "You have been unsubscribed from this newsletter.".into(),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Import {
    csv: String,
    consent_attested: bool,
    consent_source: String,
}
async fn import_contacts(
    State(app): State<App>,
    Path(id): Path<Uuid>,
    Json(p): Json<Import>,
) -> Result<Json<Value>> {
    newsletter(&app, id).await?;
    if !p.consent_attested
        || p.consent_source.trim().len() < 3
        || p.consent_source.len() > 500
        || p.csv.len() > 1_000_000
    {
        return Err(Error::bad(
            "Attest existing opt-in and provide its source; CSV maximum is 1 MB",
        ));
    }
    let mut reader = csv::Reader::from_reader(p.csv.as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| Error::bad("Invalid CSV"))?
        .clone();
    let ix = headers
        .iter()
        .position(|s| s == "email")
        .ok_or(Error::bad("CSV needs an email column"))?;
    let mut emails = std::collections::BTreeSet::new();
    for row in reader.records() {
        let row = row.map_err(|_| Error::bad("Invalid CSV row"))?;
        emails.insert(mail::email(row.get(ix).unwrap_or(""))?);
        if emails.len() > 10_000 {
            return Err(Error::bad("Import at most 10,000 contacts per request"));
        }
    }
    let mut tx = app.db.begin().await?;
    sqlx::query("SELECT id FROM newsletters WHERE id=$1 FOR UPDATE")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let mut added = 0;
    for email in emails {
        let contact:Uuid=sqlx::query_scalar("INSERT INTO contacts(id,email) VALUES($1,$2) ON CONFLICT(email) DO UPDATE SET email=EXCLUDED.email RETURNING id").bind(Uuid::new_v4()).bind(email).fetch_one(&mut *tx).await?;
        // Imports cannot undo an unsubscribe or global suppression. Rejoin requires a new public confirmation.
        added+=sqlx::query("INSERT INTO subscriptions(id,newsletter_id,contact_id,status,consent_source,consent_at) SELECT $1,$2,$3,'active',$4,now() WHERE NOT EXISTS(SELECT 1 FROM contacts WHERE id=$3 AND suppressed) ON CONFLICT(newsletter_id,contact_id) DO NOTHING").bind(Uuid::new_v4()).bind(id).bind(contact).bind(&p.consent_source).execute(&mut *tx).await?.rows_affected();
    }
    tx.commit().await?;
    Ok(Json(
        json!({"added":added,"message":"Existing subscriptions and suppressed addresses were preserved"}),
    ))
}
async fn contacts(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    newsletter(&app, id).await?;
    let rows=sqlx::query("SELECT c.email,s.status,c.suppressed,s.consent_source,s.consent_at FROM subscriptions s JOIN contacts c ON c.id=s.contact_id WHERE newsletter_id=$1 ORDER BY c.email LIMIT 10001").bind(id).fetch_all(&app.db).await?;
    if rows.len() > 10_000 {
        return Err(Error::bad(
            "This export exceeds 10,000 contacts; use a database export",
        ));
    }
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record([
        "email",
        "status",
        "suppressed",
        "consent_source",
        "consent_at",
    ])
    .map_err(|_| Error::bad("CSV encoding failed"))?;
    let mut active = 0;
    for r in &rows {
        let status: String = r.get("status");
        let suppressed: bool = r.get("suppressed");
        if status == "active" && !suppressed {
            active += 1;
        }
        let safe = |s: String| {
            if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
                format!("'{s}")
            } else {
                s
            }
        };
        w.write_record([
            safe(r.get("email")),
            status,
            suppressed.to_string(),
            safe(r.get("consent_source")),
            r.get::<Option<DateTime<Utc>>, _>("consent_at")
                .map(|v| v.to_rfc3339())
                .unwrap_or_default(),
        ])
        .map_err(|_| Error::bad("CSV encoding failed"))?;
    }
    let csv = String::from_utf8(
        w.into_inner()
            .map_err(|_| Error::bad("CSV encoding failed"))?,
    )
    .unwrap();
    Ok(Json(json!({"total":rows.len(),"active":active,"csv":csv})))
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NewCampaign {
    newsletter_id: Uuid,
    external_id: Uuid,
    subject: String,
    preview_text: String,
    markdown: String,
}
async fn create_campaign(
    State(app): State<App>,
    Json(p): Json<NewCampaign>,
) -> Result<Json<Value>> {
    let n = newsletter(&app, p.newsletter_id).await?;
    mail::render(
        &p.subject,
        &p.preview_text,
        &p.markdown,
        &n.name,
        &n.postal_address,
        Some(&format!(
            "{}/unsubscribe/{}",
            app.config.public_url,
            mail::token(&app.config.signing_key, Uuid::nil())
        )),
    )?;
    let revision = digest(&serde_json::to_vec(&p).unwrap());
    let mut tx = app.db.begin().await?;
    // Serialize create/update with scheduling; changed content can only replace an unscheduled draft.
    sqlx::query("SELECT id FROM newsletters WHERE id=$1 FOR UPDATE")
        .bind(n.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO campaigns(id,newsletter_id,external_id,subject,preview_text,markdown,revision) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(newsletter_id,external_id) DO NOTHING")
        .bind(Uuid::new_v4()).bind(n.id).bind(p.external_id).bind(&p.subject).bind(&p.preview_text).bind(&p.markdown).bind(&revision).execute(&mut *tx).await?;
    let c:Campaign=sqlx::query_as("SELECT id,newsletter_id,external_id,subject,preview_text,markdown,revision,status,scheduled_at FROM campaigns WHERE newsletter_id=$1 AND external_id=$2 FOR UPDATE").bind(n.id).bind(p.external_id).fetch_one(&mut *tx).await?;
    if c.revision != revision {
        if c.status != "draft" {
            return Err(Error::conflict(
                "This article already has a frozen campaign; inspect it before creating another edition",
            ));
        }
        sqlx::query(
            "UPDATE campaigns SET subject=$2,preview_text=$3,markdown=$4,revision=$5 WHERE id=$1",
        )
        .bind(c.id)
        .bind(p.subject)
        .bind(p.preview_text)
        .bind(p.markdown)
        .bind(revision)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    campaign_status(State(app), Path(c.id)).await
}
pub async fn audience(tx: &mut sqlx::PgConnection, id: Uuid) -> Result<Vec<(Uuid, String)>> {
    Ok(sqlx::query_as("SELECT s.id,c.email FROM subscriptions s JOIN contacts c ON c.id=s.contact_id WHERE s.newsletter_id=$1 AND s.status='active' AND NOT c.suppressed ORDER BY s.id").bind(id).fetch_all(tx).await?)
}
async fn campaign_status(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    let c = campaign(&app, id).await?;
    let rows=sqlx::query("SELECT state,count(*) AS count FROM deliveries WHERE campaign_id=$1 AND kind='campaign' GROUP BY state").bind(id).fetch_all(&app.db).await?;
    let counts: serde_json::Map<String, Value> = rows
        .into_iter()
        .map(|r| (r.get("state"), json!(r.get::<i64, _>("count"))))
        .collect();
    Ok(Json(
        json!({"campaign":c,"counts":counts,"transport":app.config.transport,"sending_enabled":app.config.sending_enabled}),
    ))
}
async fn review(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    let c = campaign(&app, id).await?;
    let n = newsletter(&app, c.newsletter_id).await?;
    let people = audience(&mut *app.db.acquire().await?, n.id).await?;
    let link = format!(
        "{}/unsubscribe/{}",
        app.config.public_url,
        mail::token(&app.config.signing_key, Uuid::nil())
    );
    let (html, plain) = mail::render(
        &c.subject,
        &c.preview_text,
        &c.markdown,
        &n.name,
        &n.postal_address,
        Some(&link),
    )?;
    Ok(Json(
        json!({"campaign":c,"newsletter":n,"audience_count":people.len(),"audience_hash":digest(&serde_json::to_vec(&people).unwrap()),"html":html,"plain":plain,"test_recipient":app.config.test_recipient,"transport":app.config.transport,"sending_enabled":app.config.sending_enabled}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Schedule {
    revision: String,
    audience_hash: String,
    scheduled_at: DateTime<Utc>,
}
async fn schedule(
    State(app): State<App>,
    Path(id): Path<Uuid>,
    Json(p): Json<Schedule>,
) -> Result<Json<Value>> {
    if !app.config.sending_enabled {
        return Err(Error::bad("Sending is disabled on this service"));
    }
    let c = campaign(&app, id).await?;
    let n = newsletter(&app, c.newsletter_id).await?;
    let mut tx = app.db.begin().await?;
    sqlx::query("SELECT id FROM newsletters WHERE id=$1 FOR UPDATE")
        .bind(n.id)
        .execute(&mut *tx)
        .await?;
    let row =
        sqlx::query("SELECT status,revision,scheduled_at FROM campaigns WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
    let status: String = row.get("status");
    let c: Campaign = sqlx::query_as("SELECT id,newsletter_id,external_id,subject,preview_text,markdown,revision,status,scheduled_at FROM campaigns WHERE id=$1").bind(id).fetch_one(&mut *tx).await?;
    if row.get::<String, _>("revision") != p.revision {
        return Err(Error::conflict("Draft changed; review it again"));
    }
    if status != "draft" {
        if matches!(
            status.as_str(),
            "scheduled" | "sending" | "complete" | "paused"
        ) && row.get::<Option<DateTime<Utc>>, _>("scheduled_at") == Some(p.scheduled_at)
        {
            drop(tx);
            return campaign_status(State(app), Path(id)).await;
        }
        return Err(Error::conflict(
            "Campaign is already frozen; refresh its status",
        ));
    }
    if p.scheduled_at < Utc::now() - chrono::Duration::minutes(5)
        || p.scheduled_at > Utc::now() + chrono::Duration::days(366)
    {
        return Err(Error::bad("Schedule within the coming year"));
    }
    let people = audience(&mut tx, n.id).await?;
    if people.is_empty()
        || people.len() > 10_000
        || digest(&serde_json::to_vec(&people).unwrap()) != p.audience_hash
    {
        return Err(Error::conflict(
            "Audience empty, changed or over 10,000; review it again",
        ));
    }
    for (sub, email) in people {
        let link = format!(
            "{}/unsubscribe/{}",
            app.config.public_url,
            mail::token(&app.config.signing_key, sub)
        );
        let (html, plain) = mail::render(
            &c.subject,
            &c.preview_text,
            &c.markdown,
            &n.name,
            &n.postal_address,
            Some(&link),
        )?;
        sqlx::query("INSERT INTO deliveries(id,dedupe_key,newsletter_id,campaign_id,subscription_id,recipient,kind,subject,html,plain,unsubscribe_url,due_at) VALUES($1,$2,$3,$4,$5,$6,'campaign',$7,$8,$9,$10,$11)")
            .bind(Uuid::new_v4()).bind(format!("{id}:{sub}")).bind(n.id).bind(id).bind(sub).bind(email).bind(&c.subject).bind(html).bind(plain).bind(link).bind(p.scheduled_at).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE campaigns SET status='scheduled',scheduled_at=$2 WHERE id=$1")
        .bind(id)
        .bind(p.scheduled_at)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    campaign_status(State(app), Path(id)).await
}
async fn cancel(State(app): State<App>, Path(id): Path<Uuid>) -> Result<Json<Value>> {
    let c = campaign(&app, id).await?;
    let mut tx = app.db.begin().await?;
    sqlx::query("SELECT id FROM newsletters WHERE id=$1 FOR UPDATE")
        .bind(c.newsletter_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT id FROM campaigns WHERE id=$1 FOR UPDATE")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE deliveries SET state='cancelled' WHERE campaign_id=$1 AND state='pending'")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE campaigns SET status='cancelled' WHERE id=$1 AND status IN ('draft','scheduled','sending','paused')").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    campaign_status(State(app), Path(id)).await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Test {
    request_id: Uuid,
    revision: String,
}
async fn test_send(
    State(app): State<App>,
    Path(id): Path<Uuid>,
    Json(p): Json<Test>,
) -> Result<Json<Value>> {
    if !app.config.sending_enabled {
        return Err(Error::bad("Sending is disabled on this service"));
    }
    let c = campaign(&app, id).await?;
    let n = newsletter(&app, c.newsletter_id).await?;
    if c.revision != p.revision {
        return Err(Error::conflict("Draft changed; review again"));
    }
    limit(&app, "test-send", 30).await?;
    let (html, plain) = mail::render(
        &c.subject,
        &c.preview_text,
        &c.markdown,
        &n.name,
        &n.postal_address,
        None,
    )?;
    let delivery:Uuid=sqlx::query_scalar("INSERT INTO deliveries(id,dedupe_key,newsletter_id,campaign_id,recipient,kind,subject,html,plain,due_at) VALUES($1,$2,$3,$4,$5,'test',$6,$7,$8,now()) ON CONFLICT(dedupe_key) DO UPDATE SET dedupe_key=EXCLUDED.dedupe_key RETURNING id")
        .bind(Uuid::new_v4()).bind(format!("test:{id}:{}:{}",c.revision,p.request_id)).bind(n.id).bind(id).bind(&app.config.test_recipient).bind(format!("[Test] {}",c.subject)).bind(html).bind(plain).fetch_one(&app.db).await?;
    Ok(Json(
        json!({"delivery_id":delivery,"recipient":app.config.test_recipient,"transport":app.config.transport}),
    ))
}
async fn upload_asset(
    State(app): State<App>,
    headers: HeaderMap,
    bytes: Bytes,
) -> Result<Json<Value>> {
    let mime = headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if bytes.len() > 5 * 1024 * 1024
        || !((mime == "image/png" && bytes.starts_with(b"\x89PNG\r\n\x1a\n"))
            || (mime == "image/jpeg" && bytes.starts_with(b"\xff\xd8\xff")))
    {
        return Err(Error::bad("Upload a PNG or JPEG up to 5 MiB"));
    }
    let hash = digest(&bytes);
    sqlx::query("INSERT INTO assets(digest,mime,bytes) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
        .bind(&hash)
        .bind(mime)
        .bind(&*bytes)
        .execute(&app.db)
        .await?;
    Ok(Json(
        json!({"url":format!("{}/assets/{hash}",app.config.public_url)}),
    ))
}
async fn asset(State(app): State<App>, Path(hash): Path<String>) -> Result<Response> {
    if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::bad("Invalid image ID"));
    }
    let row = sqlx::query("SELECT mime,bytes FROM assets WHERE digest=$1")
        .bind(hash)
        .fetch_optional(&app.db)
        .await?
        .ok_or(Error(StatusCode::NOT_FOUND, "Image not found".into()))?;
    Ok((
        [("content-type", row.get::<String, _>("mime"))],
        row.get::<Vec<u8>, _>("bytes"),
    )
        .into_response())
}
async fn captures(State(app): State<App>) -> Result<Json<Value>> {
    if app.config.transport != "capture" {
        return Err(Error(
            StatusCode::NOT_FOUND,
            "Capture transport is not active".into(),
        ));
    }
    let rows: Vec<Value> = sqlx::query_scalar(
        "SELECT to_jsonb(d) FROM deliveries d ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&app.db)
    .await?;
    Ok(Json(
        json!({"deliveries":rows,"message":"Capture mode: no email leaves the service"}),
    ))
}
