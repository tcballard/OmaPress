//! A claimed SES send is never blindly retried. Provider events can resolve uncertainty.
use crate::{App, api};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use sqlx::Row;
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncWriteExt, process::Command};
use uuid::Uuid;

async fn aws(app: &App, service: &str, operation: &str, payload: Value) -> Result<Value> {
    let mut child = Command::new("aws")
        .args([
            service,
            operation,
            "--region",
            &app.config.ses_region,
            "--cli-input-json",
            "file:///dev/stdin",
            "--output",
            "json",
            "--no-cli-pager",
            "--cli-connect-timeout",
            "5",
            "--cli-read-timeout",
            "20",
        ])
        .env("AWS_MAX_ATTEMPTS", "1")
        .env("AWS_RETRY_MODE", "standard")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("AWS CLI unavailable")?;
    let mut stdin = child.stdin.take().context("AWS CLI stdin unavailable")?;
    let body = serde_json::to_vec(&payload)?;
    let task = async move {
        stdin.write_all(&body).await?;
        drop(stdin);
        child.wait_with_output().await
    };
    let out = tokio::time::timeout(Duration::from_secs(30), task)
        .await
        .context("AWS operation timed out; outcome unknown")??;
    // Do not expose provider stderr: it can contain addresses or request content.
    ensure!(
        out.status.success(),
        "AWS operation failed; inspect provider events before retrying"
    );
    ensure!(
        out.stdout.len() < 12 * 1024 * 1024,
        "Provider response too large"
    );
    if out.stdout.is_empty() {
        Ok(Value::Null)
    } else {
        Ok(serde_json::from_slice(&out.stdout)?)
    }
}
async fn poll_events(app: &App) -> Result<()> {
    if app.config.transport != "ses" {
        return Ok(());
    }
    let messages=aws(app,"sqs","receive-message",json!({"QueueUrl":app.config.events_queue_url,"MaxNumberOfMessages":10,"WaitTimeSeconds":0,"VisibilityTimeout":60})).await?;
    if let Some(messages) = messages["Messages"].as_array() {
        for message in messages {
            let envelope: Value =
                serde_json::from_str(message["Body"].as_str().context("Missing SQS body")?)?;
            ensure!(
                envelope["TopicArn"] == app.config.events_topic_arn
                    && envelope["Type"] == "Notification",
                "Unexpected event origin; check SNS/SQS policy"
            );
            let event: Value = serde_json::from_str(
                envelope["Message"]
                    .as_str()
                    .context("Missing SNS message")?,
            )?;
            apply_event(
                app,
                envelope["MessageId"].as_str().context("Missing event ID")?,
                &event,
            )
            .await?;
            aws(app,"sqs","delete-message",json!({"QueueUrl":app.config.events_queue_url,"ReceiptHandle":message["ReceiptHandle"]})).await?;
        }
    }
    Ok(())
}
pub async fn apply_event(app: &App, event_id: &str, event: &Value) -> Result<()> {
    let kind = event["eventType"]
        .as_str()
        .or(event["notificationType"].as_str())
        .unwrap_or("");
    let Some(tag) = event["mail"]["tags"]["newsletter_delivery"][0].as_str() else {
        return Ok(());
    };
    let id = Uuid::parse_str(tag)?;
    let provider = event["mail"]["messageId"]
        .as_str()
        .context("Event lacks message ID")?;
    let state = match kind {
        "Send" => "accepted",
        "Delivery" => "delivered",
        "Bounce" => "bounced",
        "Complaint" => "complained",
        "Reject" | "Rendering Failure" => "failed",
        _ => return Ok(()),
    };
    let mut tx = app.db.begin().await?;
    let inserted = sqlx::query("INSERT INTO provider_events(id) VALUES($1) ON CONFLICT DO NOTHING")
        .bind(event_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if inserted == 0 {
        return Ok(());
    }
    // Match cancellation/subscription lock order before touching deliveries.
    let owner = sqlx::query("SELECT newsletter_id,campaign_id FROM deliveries WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(owner) = owner else {
        return Ok(());
    };
    sqlx::query("SELECT id FROM newsletters WHERE id=$1 FOR UPDATE")
        .bind(owner.get::<Uuid, _>("newsletter_id"))
        .execute(&mut *tx)
        .await?;
    if let Some(campaign) = owner.get::<Option<Uuid>, _>("campaign_id") {
        sqlx::query("SELECT id FROM campaigns WHERE id=$1 FOR UPDATE")
            .bind(campaign)
            .execute(&mut *tx)
            .await?;
    }
    let Some(row) = sqlx::query(
        "SELECT recipient,campaign_id,state,provider_id FROM deliveries WHERE id=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(());
    };
    if let Some(existing) = row.get::<Option<String>, _>("provider_id") {
        ensure!(
            existing == provider,
            "Event has mismatched provider message ID"
        );
    }
    let recipient: String = row.get("recipient");
    if matches!(state, "bounced" | "complained") {
        let recipients = if state == "bounced" {
            &event["bounce"]["bouncedRecipients"]
        } else {
            &event["complaint"]["complainedRecipients"]
        };
        ensure!(
            recipients
                .as_array()
                .is_some_and(|rs| rs.iter().any(|r| r["emailAddress"]
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case(&recipient)))),
            "Event recipient does not match delivery"
        );
        sqlx::query("UPDATE contacts SET suppressed=true,suppression_reason=$2 WHERE email=$1")
            .bind(&recipient)
            .bind(state)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE deliveries SET state='suppressed' WHERE recipient=$1 AND state='pending'",
        )
        .bind(&recipient)
        .execute(&mut *tx)
        .await?;
    }
    let rank = |s: &str| match s {
        "complained" => 6,
        "bounced" => 5,
        "failed" => 4,
        "delivered" => 3,
        "accepted" => 2,
        _ => 1,
    };
    if rank(state) >= rank(&row.get::<String, _>("state")) {
        sqlx::query("UPDATE deliveries SET state=$2,provider_id=$3,last_error=NULL WHERE id=$1")
            .bind(id)
            .bind(state)
            .bind(provider)
            .execute(&mut *tx)
            .await?;
    }
    if let Some(campaign) = row.get::<Option<Uuid>, _>("campaign_id") {
        sqlx::query("UPDATE campaigns SET status='paused' WHERE id=$1 AND status IN ('scheduled','sending') AND EXISTS(SELECT 1 FROM deliveries WHERE campaign_id=$1 AND kind='campaign' HAVING count(*) FILTER(WHERE state='bounced')::float/GREATEST(count(*) FILTER(WHERE state IN ('accepted','delivered','bounced','complained','failed','unknown','dispatching')),1)>0.02 OR count(*) FILTER(WHERE state='complained')::float/GREATEST(count(*) FILTER(WHERE state IN ('accepted','delivered','bounced','complained','failed','unknown','dispatching')),1)>0.0008)").bind(campaign).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
/// One pass, used by the owned worker and integration tests. A transaction-scoped
/// advisory lock serializes workers across replicas and caps the aggregate rate.
pub async fn tick(app: &App) -> Result<bool> {
    let mut lease = app.db.begin().await?;
    let held: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(781264390)")
        .fetch_one(&mut *lease)
        .await?;
    if !held {
        return Ok(false);
    }
    poll_events(app).await?; // Fail closed if feedback processing is unavailable.
    sqlx::query("UPDATE deliveries SET state='unknown',last_error='Worker interrupted after dispatch claim; awaiting provider evidence' WHERE state='dispatching' AND started_at<now()-interval '2 minutes'").execute(&app.db).await?;
    if !app.config.sending_enabled {
        return Ok(false);
    }
    let mut tx = app.db.begin().await?;
    // Choose the campaign before locking its deliveries: same lock order as cancel.
    let next=sqlx::query("SELECT d.id,d.campaign_id FROM deliveries d LEFT JOIN campaigns c ON c.id=d.campaign_id WHERE d.state='pending' AND d.due_at<=now() AND (d.kind!='campaign' OR c.status IN ('scheduled','sending')) ORDER BY d.due_at,d.id LIMIT 1").fetch_optional(&mut *tx).await?;
    let Some(next) = next else {
        return Ok(false);
    };
    let id: Uuid = next.get("id");
    if let Some(campaign) = next.get::<Option<Uuid>, _>("campaign_id") {
        let state: String =
            sqlx::query_scalar("SELECT status FROM campaigns WHERE id=$1 FOR UPDATE")
                .bind(campaign)
                .fetch_one(&mut *tx)
                .await?;
        if !matches!(
            state.as_str(),
            "scheduled" | "sending" | "draft" | "complete" | "cancelled" | "paused"
        ) {
            return Ok(false);
        }
    }
    let row = sqlx::query("SELECT * FROM deliveries WHERE id=$1 AND state='pending' FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let kind: String = row.get("kind");
    let campaign = row.get::<Option<Uuid>, _>("campaign_id");
    if kind == "campaign" {
        let status: String = sqlx::query_scalar("SELECT status FROM campaigns WHERE id=$1")
            .bind(campaign)
            .fetch_one(&mut *tx)
            .await?;
        if !matches!(status.as_str(), "scheduled" | "sending") {
            return Ok(false);
        }
    }
    let recipient: String = row.get("recipient");
    let suppressed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM contacts WHERE email=$1 AND suppressed)")
            .bind(&recipient)
            .fetch_one(&mut *tx)
            .await?;
    let eligible = if kind == "campaign" {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM subscriptions WHERE id=$1 AND status='active')",
        )
        .bind(row.get::<Option<Uuid>, _>("subscription_id"))
        .fetch_one(&mut *tx)
        .await?
    } else {
        true
    };
    if suppressed || !eligible {
        sqlx::query("UPDATE deliveries SET state='suppressed' WHERE id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok(true);
    }
    sqlx::query("UPDATE deliveries SET state='dispatching',started_at=now() WHERE id=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if kind == "campaign" {
        sqlx::query("UPDATE campaigns SET status='sending' WHERE id=$1")
            .bind(campaign)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?; // Crash from here onwards leaves an uncertain outcome, never a retry.
    let result = if app.config.transport == "capture" {
        Ok(format!("capture:{id}"))
    } else {
        send(app, &row).await
    };
    match result {
        Ok(provider) => {
            sqlx::query(
                "UPDATE deliveries SET state=$2,provider_id=$3 WHERE id=$1 AND state='dispatching'",
            )
            .bind(id)
            .bind(if app.config.transport == "capture" {
                "captured"
            } else {
                "accepted"
            })
            .bind(provider)
            .execute(&app.db)
            .await?;
        }
        Err(_) => {
            sqlx::query("UPDATE deliveries SET state='unknown',last_error='Provider result uncertain; do not resend without reconciliation' WHERE id=$1 AND state='dispatching'").bind(id).execute(&app.db).await?;
        }
    }
    sqlx::query("UPDATE campaigns SET status='complete' WHERE status IN ('scheduled','sending') AND NOT EXISTS(SELECT 1 FROM deliveries WHERE campaign_id=campaigns.id AND kind='campaign' AND state IN ('pending','dispatching'))").execute(&app.db).await?;
    tokio::time::sleep(Duration::from_millis(app.config.send_interval_ms)).await;
    lease.commit().await?;
    Ok(true)
}
async fn send(app: &App, row: &sqlx::postgres::PgRow) -> Result<String> {
    let n = api::newsletter(app, row.get("newsletter_id"))
        .await
        .map_err(|_| anyhow::anyhow!("Sender unavailable"))?;
    let id: Uuid = row.get("id");
    let mut headers = vec![];
    if let Some(url) = row.get::<Option<String>, _>("unsubscribe_url") {
        headers.push(json!({"Name":"List-Unsubscribe","Value":format!("<{url}>")}));
        headers.push(json!({"Name":"List-Unsubscribe-Post","Value":"List-Unsubscribe=One-Click"}));
    }
    let result=aws(app,"sesv2","send-email",json!({
        "FromEmailAddress":n.from_email,"ReplyToAddresses":[n.reply_to],
        "Destination":{"ToAddresses":[row.get::<String,_>("recipient")]},
        "ConfigurationSetName":app.config.ses_configuration_set,
        "EmailTags":[{"Name":"newsletter_delivery","Value":id.to_string()}],
        "Content":{"Simple":{"Subject":{"Data":row.get::<String,_>("subject"),"Charset":"UTF-8"},"Body":{"Html":{"Data":row.get::<String,_>("html"),"Charset":"UTF-8"},"Text":{"Data":row.get::<String,_>("plain"),"Charset":"UTF-8"}},"Headers":headers}}
    })).await?;
    Ok(result["MessageId"]
        .as_str()
        .context("SES omitted message ID")?
        .to_string())
}
pub async fn run(app: App, mut stop: tokio::sync::watch::Receiver<bool>) {
    loop {
        if *stop.borrow() {
            break;
        }
        // Finish the current attempt on shutdown. Its provider call is bounded to 30s.
        if tick(&app).await.is_err() {
            eprintln!(
                "Newsletter worker paused this pass; inspect database/provider availability (no recipient data logged)"
            );
        }
        tokio::select! { _=stop.changed()=>{}, _=tokio::time::sleep(Duration::from_secs(1))=>{} }
    }
}
