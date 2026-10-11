use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use newsletter_service::{App, api, config::Config, worker};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

fn app(db: PgPool) -> App {
    App {
        db,
        config: Arc::new(Config {
            public_url: "https://newsletter.example.com".into(),
            admin_token: "test-admin-token-at-least-thirty-two-characters".into(),
            signing_key: "different-signing-key-at-least-thirty-two-characters".into(),
            transport: "capture".into(),
            sending_enabled: true,
            test_recipient: "author@example.com".into(),
            ses_region: "eu-west-2".into(),
            ses_configuration_set: String::new(),
            events_queue_url: String::new(),
            events_topic_arn: String::new(),
            send_interval_ms: 100,
        }),
    }
}
async fn req(router: &Router, method: &str, path: &str, body: Value) -> (u16, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header(
            "authorization",
            "Bearer test-admin-token-at-least-thirty-two-characters",
        )
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 12 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(json!({"text":String::from_utf8_lossy(&bytes)})),
    )
}
async fn form(router: &Router, method: &str, path: &str, body: &str) -> u16 {
    router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
        .as_u16()
}
async fn create(router: &Router) -> String {
    let (code,n)=req(router,"POST","/v1/newsletters",json!({"slug":"weekly","name":"Weekly","from_email":"author@example.com","reply_to":"reply@example.com","postal_address":"Test postal address"})).await;
    assert_eq!(code, 200, "{n}");
    n["newsletter"]["id"].as_str().unwrap().into()
}
async fn import(router: &Router, id: &str, email: &str) {
    let (code,v)=req(router,"POST",&format!("/v1/newsletters/{id}/contacts"),json!({"csv":format!("email\n{email}\n"),"consent_attested":true,"consent_source":"Recorded opt-in from previous newsletter"})).await;
    assert_eq!(code, 200, "{v}");
}
async fn campaign(router: &Router, id: &str, external: Uuid) -> Value {
    let (code,c)=req(router,"POST","/v1/campaigns",json!({"newsletter_id":id,"external_id":external,"subject":"A newsletter","preview_text":"Preview","markdown":"## Hello\n\nA real **article**."})).await;
    assert_eq!(code, 200, "{c}");
    c["campaign"].clone()
}
async fn review(router: &Router, id: &str) -> Value {
    let (code, v) = req(
        router,
        "GET",
        &format!("/v1/campaigns/{id}/review"),
        Value::Null,
    )
    .await;
    assert_eq!(code, 200, "{v}");
    v
}

#[sqlx::test(migrations = "./migrations")]
async fn consent_freeze_and_delivery(pool: PgPool) {
    let app = app(pool);
    let router = api::router(app.clone());
    let unauth = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/newsletters")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth.status(), 401);
    let n = create(&router).await;
    assert_eq!(
        form(
            &router,
            "POST",
            "/subscribe/weekly",
            "email=alice%40example.com"
        )
        .await,
        200
    );
    let confirmation: String =
        sqlx::query_scalar("SELECT plain FROM deliveries WHERE kind='confirmation'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    let token = confirmation.split("/confirm/").nth(1).unwrap();
    assert_eq!(
        form(&router, "GET", &format!("/confirm/{token}"), "").await,
        200
    );
    let pending: String = sqlx::query_scalar("SELECT status FROM subscriptions")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(pending, "pending", "Link scanners must not confirm opt-in");
    assert_eq!(
        form(&router, "POST", &format!("/confirm/{token}"), "").await,
        200
    );
    // Drain confirmation separately from the campaign.
    worker::tick(&app).await.unwrap();
    let external = Uuid::new_v4();
    let c = campaign(&router, &n, external).await;
    let id = c["id"].as_str().unwrap();
    assert_eq!(
        campaign(&router, &n, external).await["id"],
        c["id"],
        "Creation retries reuse the same campaign"
    );
    let old = review(&router, id).await;
    import(&router, &n, "bob@example.com").await;
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let payload =
        json!({"revision":c["revision"],"audience_hash":old["audience_hash"],"scheduled_at":at});
    assert_eq!(
        req(
            &router,
            "POST",
            &format!("/v1/campaigns/{id}/schedule"),
            payload
        )
        .await
        .0,
        409
    );
    let v = review(&router, id).await;
    assert_eq!(v["audience_count"], 2);
    let payload =
        json!({"revision":c["revision"],"audience_hash":v["audience_hash"],"scheduled_at":at});
    let path = format!("/v1/campaigns/{id}/schedule");
    let (a, b) = tokio::join!(
        req(&router, "POST", &path, payload.clone()),
        req(&router, "POST", &path, payload)
    );
    assert_eq!(a.0, 200, "{}", a.1);
    assert_eq!(b.0, 200, "{}", b.1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM deliveries WHERE kind='campaign'")
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(count, 2);
    let unsubscribe:String=sqlx::query_scalar("SELECT unsubscribe_url FROM deliveries WHERE recipient='alice@example.com' AND kind='campaign'").fetch_one(&app.db).await.unwrap();
    let path = unsubscribe.strip_prefix(&app.config.public_url).unwrap();
    assert_eq!(form(&router, "GET", path, "").await, 200);
    assert_eq!(
        form(&router, "POST", path, "List-Unsubscribe=One-Click").await,
        200
    );
    import(&router, &n, "alice@example.com").await;
    let status:String=sqlx::query_scalar("SELECT status FROM subscriptions s JOIN contacts c ON c.id=s.contact_id WHERE c.email='alice@example.com'").fetch_one(&app.db).await.unwrap();
    assert_eq!(status, "unsubscribed");
    let (a, b) = tokio::join!(worker::tick(&app), worker::tick(&app));
    a.unwrap();
    b.unwrap();
    let counts = sqlx::query("SELECT recipient,state FROM deliveries WHERE kind='campaign'")
        .fetch_all(&app.db)
        .await
        .unwrap();
    for r in counts {
        assert_eq!(
            r.get::<String, _>("state"),
            if r.get::<String, _>("recipient") == "alice@example.com" {
                "suppressed"
            } else {
                "captured"
            }
        );
    }
    assert!(
        !worker::tick(&app).await.unwrap(),
        "Finished messages cannot be resent"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn restart_uncertainty_events_and_cancel(pool: PgPool) {
    let app = app(pool);
    let router = api::router(app.clone());
    let n = create(&router).await;
    import(&router, &n, "alice@example.com").await;
    import(&router, &n, "bob@example.com").await;
    let c = campaign(&router, &n, Uuid::new_v4()).await;
    let id = c["id"].as_str().unwrap();
    let r = review(&router, id).await;
    let payload = json!({"revision":c["revision"],"audience_hash":r["audience_hash"],"scheduled_at":chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs,true)});
    assert_eq!(
        req(
            &router,
            "POST",
            &format!("/v1/campaigns/{id}/schedule"),
            payload
        )
        .await
        .0,
        200
    );
    // Simulate a crash between the durable dispatch claim and the SES acknowledgement.
    let delivery:Uuid=sqlx::query_scalar("UPDATE deliveries SET state='dispatching',started_at=now()-interval '3 minutes' WHERE recipient='alice@example.com' RETURNING id").fetch_one(&app.db).await.unwrap();
    worker::tick(&app).await.unwrap();
    let state: String = sqlx::query_scalar("SELECT state FROM deliveries WHERE id=$1")
        .bind(delivery)
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(state, "unknown");
    let event = json!({"eventType":"Delivery","mail":{"messageId":"ses-1","tags":{"newsletter_delivery":[delivery.to_string()]}}});
    worker::apply_event(&app, "event-1", &event).await.unwrap();
    worker::apply_event(&app, "event-1", &event).await.unwrap();
    let bounce = json!({"eventType":"Bounce","mail":{"messageId":"ses-1","tags":{"newsletter_delivery":[delivery.to_string()]}},"bounce":{"bouncedRecipients":[{"emailAddress":"alice@example.com"}]}});
    worker::apply_event(&app, "event-2", &bounce).await.unwrap();
    worker::apply_event(&app, "event-3", &event).await.unwrap();
    let state: String = sqlx::query_scalar("SELECT state FROM deliveries WHERE id=$1")
        .bind(delivery)
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(
        state, "bounced",
        "Late delivery events cannot undo a bounce"
    );
    let suppressed: bool =
        sqlx::query_scalar("SELECT suppressed FROM contacts WHERE email='alice@example.com'")
            .fetch_one(&app.db)
            .await
            .unwrap();
    assert!(suppressed);
    let new = campaign(&router, &n, Uuid::new_v4()).await;
    let id = new["id"].as_str().unwrap();
    let r = review(&router, id).await;
    assert_eq!(r["audience_count"], 1);
    let payload = json!({"revision":new["revision"],"audience_hash":r["audience_hash"],"scheduled_at":(chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339_opts(chrono::SecondsFormat::Secs,true)});
    assert_eq!(
        req(
            &router,
            "POST",
            &format!("/v1/campaigns/{id}/schedule"),
            payload
        )
        .await
        .0,
        200
    );
    assert_eq!(
        req(
            &router,
            "POST",
            &format!("/v1/campaigns/{id}/cancel"),
            json!({})
        )
        .await
        .0,
        200
    );
    let state: String = sqlx::query_scalar("SELECT state FROM deliveries WHERE campaign_id=$1")
        .bind(Uuid::parse_str(id).unwrap())
        .fetch_one(&app.db)
        .await
        .unwrap();
    assert_eq!(state, "cancelled");
}
