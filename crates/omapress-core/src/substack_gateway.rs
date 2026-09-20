//! Optional client of jakub-k-slys/substack-gateway-oss, not an official Substack SDK.
use crate::{
    distribution::{self, Receipt},
    model::*,
    process,
    storage::*,
};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    gateway_url: String,
    publication_url: String,
    substack_sid: String,
    #[serde(default)]
    connect_sid: String,
}
fn validate(c: &Config) -> Result<()> {
    let u = url::Url::parse(&c.gateway_url)?;
    ensure!(
        (u.scheme() == "https"
            || (u.scheme() == "http"
                && ["127.0.0.1", "[::1]"].contains(&u.host_str().unwrap_or(""))))
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none()
            && u.path() == "/",
        "Gateway must be an HTTPS origin or numeric loopback HTTP origin"
    );
    let p = url::Url::parse(&c.publication_url)?;
    ensure!(
        p.scheme() == "https"
            && p.host_str().is_some_and(|h| h.ends_with(".substack.com"))
            && p.username().is_empty()
            && p.password().is_none()
            && p.port().is_none()
            && p.path() == "/"
            && p.query().is_none()
            && p.fragment().is_none(),
        "Use your https://publication.substack.com origin"
    );
    ensure!(
        !c.substack_sid.is_empty()
            && c.substack_sid.len() < 8192
            && c.connect_sid.len() < 8192
            && !c
                .substack_sid
                .chars()
                .chain(c.connect_sid.chars())
                .any(char::is_control),
        "Invalid Substack session credentials"
    );
    Ok(())
}
fn credential_file() -> Result<Option<PathBuf>> {
    use std::os::unix::fs::MetadataExt;
    let Some(path) = std::env::var_os("OMAPRESS_SUBSTACK_GATEWAY_CREDENTIALS_FILE")
        .or_else(|| std::env::var_os("PRESSROOM_SUBSTACK_GATEWAY_CREDENTIALS_FILE"))
    else {
        return Ok(None);
    };
    let p = PathBuf::from(path);
    ensure!(p.is_absolute(), "Gateway credential path must be absolute");
    let m = std::fs::symlink_metadata(&p)?;
    ensure!(
        m.is_file() && m.mode() & 0o077 == 0 && m.uid() == unsafe { libc::geteuid() },
        "Gateway credentials must be an owner-only regular file"
    );
    Ok(Some(p))
}
fn read() -> Result<Config> {
    let text = if let Some(p) = credential_file()? {
        String::from_utf8(read_bounded(&p, 32 * 1024)?)?
    } else {
        process::run(
            "secret-tool",
            &[
                "lookup",
                "application",
                "pressroom",
                "service",
                "substack-gateway",
            ],
            None,
            Duration::from_secs(15),
        )
        .map_err(|_| anyhow::anyhow!("Connect a Substack Gateway first"))?
    };
    let c: Config = serde_json::from_str(&text).context("Invalid gateway credentials")?;
    validate(&c)?;
    Ok(c)
}
pub fn connect(value: Value) -> Result<Value> {
    let c: Config = serde_json::from_value(value)?;
    validate(&c)?;
    let _lock = lock(&crate::substack::directory()?)?;
    let bytes = serde_json::to_vec(&c)?;
    if let Some(p) = credential_file()? {
        atomic_write(&p, &bytes)?;
    } else {
        process::run_input(
            "secret-tool",
            &[
                "store",
                "--label=OmaPress Substack Gateway",
                "application",
                "pressroom",
                "service",
                "substack-gateway",
            ],
            None,
            Duration::from_secs(15),
            &bytes,
        )
        .map_err(|_| anyhow::anyhow!("Cannot save gateway credentials in Secret Service"))?;
    }
    status()
}
pub fn status() -> Result<Value> {
    let c = read()?;
    Ok(
        json!({"configured":true,"gateway_url":c.gateway_url,"publication_url":c.publication_url,"message":"Credentials saved; this does not verify a live Substack session."}),
    )
}
fn request(c: &Config, method: &str, path: &str, body: Option<&Value>) -> Result<Value> {
    let token = crate::x_article::base64(&serde_json::to_vec(
        &json!({"publication_url":c.publication_url,"substack_sid":c.substack_sid,"connect_sid":c.connect_sid}),
    )?);
    let config = format!("header = \"x-gateway-token: {token}\"\n");
    let mut cfg = tempfile::NamedTempFile::new()?;
    cfg.write_all(config.as_bytes())?;
    let mut payload = tempfile::NamedTempFile::new()?;
    if let Some(b) = body {
        payload.write_all(&serde_json::to_vec(b)?)?;
    }
    let url = format!("{}/api/v1/{path}", c.gateway_url.trim_end_matches('/'));
    let data = format!("@{}", payload.path().display());
    let mut args = vec![
        "--disable",
        "--silent",
        "--fail",
        "--max-time",
        "60",
        "--proto",
        "=https,http",
        "--config",
        cfg.path().to_str().unwrap(),
        "--request",
        method,
    ];
    if body.is_some() {
        args.extend([
            "--header",
            "Content-Type: application/json",
            "--data-binary",
            &data,
        ]);
    }
    args.push(&url);
    let text=process::run("curl",&args,None,Duration::from_secs(65)).map_err(|_|anyhow::anyhow!("Gateway request failed or timed out. Inspect Substack before retrying an uncertain operation."))?;
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).context("Invalid gateway response")
}
fn draft_body(s: &Snapshot, a: &Article) -> Result<String> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    let mut edits = vec![];
    for (event, range) in Parser::new_ext(&a.body, Options::all()).into_offset_iter() {
        match event {
            Event::Html(_) | Event::InlineHtml(_) => bail!("Use the browser handoff for raw HTML"),
            Event::Start(Tag::Image { dest_url, .. }) => {
                let path = dest_url.trim_start_matches('/');
                ensure!(
                    path.starts_with("media/"),
                    "Import Substack artwork locally first"
                );
                let bytes = s.files.get(path).context("Image missing")?;
                let (_, mime) = detect_image(bytes)?;
                ensure!(
                    ["image/png", "image/jpeg"].contains(&mime) && bytes.len() <= 5 * 1024 * 1024,
                    "Gateway artwork must be PNG/JPEG up to 5 MiB"
                );
                let original = &a.body[range.clone()];
                // Preserve alt text by rebuilding only standard inline image syntax.
                let end = original
                    .find("](")
                    .context("Use inline Markdown images for gateway export")?;
                let alt = &original[2..end];
                edits.push((
                    range,
                    format!(
                        "![{alt}](data:{mime};base64,{})",
                        crate::x_article::base64(bytes)
                    ),
                ));
            }
            _ => {}
        }
    }
    let mut body = a.body.clone();
    for (range, text) in edits.into_iter().rev() {
        body.replace_range(range, &text);
    }
    if let Some(cover) = &a.meta.header_image {
        let bytes = s
            .files
            .get(cover.trim_start_matches('/'))
            .context("Import the cover locally first")?;
        let (_, mime) = detect_image(bytes)?;
        ensure!(
            ["image/png", "image/jpeg"].contains(&mime) && bytes.len() <= 5 * 1024 * 1024,
            "Cover must be PNG/JPEG up to 5 MiB"
        );
        body = format!(
            "![Cover image](data:{mime};base64,{})\n\n{body}",
            crate::x_article::base64(bytes)
        );
    }
    ensure!(
        body.len() <= 16 * 1024 * 1024,
        "Gateway export exceeds 16 MiB"
    );
    Ok(body)
}
pub fn action(root: &Path, command: &str, args: &Value) -> Result<Value> {
    let _lock = lock(root)?;
    let s = snapshot(root)?;
    s.expected(
        args["expected_source_hash"]
            .as_str()
            .context("Missing reviewed source hash")?,
    )?;
    let path = args["article"].as_str().context("Missing article")?;
    let a = parse_article(path, s.files.get(path).context("Article not found")?)?;
    let c = read()?;
    let account = hash(
        format!(
            "{}:{}",
            c.gateway_url.trim_end_matches('/'),
            c.publication_url.trim_end_matches('/')
        )
        .as_bytes(),
    );
    let id = a.meta.id.to_string();
    let previous = distribution::gateway_receipt(root, &id)?;
    let mut r = previous.clone().unwrap_or(Receipt {
        article_hash: a.hash.clone(),
        status: "gateway_new".into(),
        url: format!("{}/publish", c.publication_url.trim_end_matches('/')),
        remote_id: String::new(),
        updated_at: String::new(),
        account_id: account.clone(),
        review_hash: String::new(),
        schedule_hash: String::new(),
        scheduled_at: String::new(),
        post_audience: String::new(),
        email_audience: String::new(),
    });
    ensure!(
        r.account_id == account
            && (command == "substack-gateway-cancel" || r.article_hash == a.hash),
        "Existing Substack receipt belongs to another connection or article version; use that existing draft to avoid duplicates"
    );
    let save = |r: &Receipt| distribution::save_gateway_receipt(root, &id, r.clone());
    if command == "substack-gateway-draft" {
        ensure!(
            ["gateway_new", "gateway_draft"].contains(&r.status.as_str())
                || (r.status == "gateway_unknown"
                    && !r.remote_id.is_empty()
                    && r.schedule_hash.is_empty()),
            "Previous Substack action is scheduled, recorded or uncertain; inspect it before continuing"
        );
        if r.remote_id.is_empty() {
            ensure!(
                a.meta.status != Status::Draft
                    && !crate::render::diagnostics(&s, &state(root)?)
                        .iter()
                        .any(|d| d.severity == "error"),
                "Mark Ready and resolve publication checks"
            );
            let body = draft_body(&s, &a)?;
            r.status = "gateway_unknown".into();
            save(&r)?;
            let v = request(
                &c,
                "POST",
                "drafts",
                Some(&json!({"title":a.meta.title,"subtitle":a.meta.summary,"body":body})),
            )?;
            r.remote_id = v["id"]
                .as_u64()
                .filter(|id| *id > 0)
                .context("Gateway returned no draft ID; inspect Substack")?
                .to_string();
            // Persist the ID before readback; an interrupted GET never loses the created draft.
            save(&r)?;
        }
        let remote = request(&c, "GET", &format!("drafts/{}", r.remote_id), None)?;
        ensure!(
            remote["title"] == a.meta.title
                && remote["subtitle"] == a.meta.summary
                && remote["body"]
                    .as_str()
                    .is_some_and(|b| !b.trim().is_empty()),
            "Draft readback differs; inspect Substack"
        );
        r.review_hash = hash(&serde_json::to_vec(&remote)?);
        r.status = "gateway_draft".into();
    } else if command == "substack-gateway-schedule" {
        let at = crate::schedule::parse_time(args)?;
        ensure!(
            at.timestamp_subsec_nanos() == 0,
            "Use a release time in whole seconds"
        );
        let post = args["post_audience"]
            .as_str()
            .context("Choose post audience")?;
        let email = args["email_audience"]
            .as_str()
            .context("Choose email audience")?;
        ensure!(
            ["everyone", "only_paid"].contains(&post) && ["everyone", "only_paid"].contains(&email),
            "Choose everyone or paid subscribers explicitly"
        );
        ensure!(
            post == "everyone" || email == "only_paid",
            "A paid-only post cannot be emailed to everyone"
        );
        let payload =
            json!({"scheduled_at":at.to_rfc3339(),"post_audience":post,"email_audience":email});
        let schedule_hash = hash(&serde_json::to_vec(&payload)?);
        if r.status == "scheduled" && r.schedule_hash == schedule_hash {
            return Ok(json!({"receipt":r}));
        }
        ensure!(
            at > chrono::Utc::now(),
            "Choose a future Substack release time"
        );
        ensure!(
            r.status == "gateway_draft"
                && args["reviewed_draft_hash"] == r.review_hash
                && args["reviewed_in_substack"] == true,
            "Open and review the prepared Substack draft before scheduling"
        );
        let remote = request(&c, "GET", &format!("drafts/{}", r.remote_id), None)?;
        ensure!(
            hash(&serde_json::to_vec(&remote)?) == r.review_hash,
            "Substack draft changed after review; prepare/review it again"
        );
        let check = request(
            &c,
            "GET",
            &format!("drafts/{}/prepublish", r.remote_id),
            None,
        )?;
        ensure!(
            check["errors"].as_array().is_some_and(|a| a.is_empty()),
            "Substack pre-publish checks failed; resolve them in Substack"
        );
        r.status = "gateway_unknown".into();
        r.schedule_hash = schedule_hash;
        r.scheduled_at = at.to_rfc3339();
        r.post_audience = post.into();
        r.email_audience = email.into();
        save(&r)?;
        let response = request(
            &c,
            "POST",
            &format!("drafts/{}/schedule", r.remote_id),
            Some(&payload),
        )?;
        let received = chrono::DateTime::parse_from_rfc3339(
            response["scheduled_at"]
                .as_str()
                .context("Missing schedule acknowledgement")?,
        )?;
        ensure!(
            received.with_timezone(&chrono::Utc) == at
                && response["post_audience"] == post
                && response["email_audience"] == email,
            "Schedule acknowledgement differs; inspect Substack"
        );
        r.status = "scheduled".into();
    } else {
        ensure!(
            r.status == "scheduled",
            "Only an acknowledged schedule can be cancelled automatically"
        );
        r.status = "gateway_unknown".into();
        save(&r)?;
        request(
            &c,
            "DELETE",
            &format!("drafts/{}/schedule", r.remote_id),
            None,
        )?;
        r.status = "gateway_draft".into();
        r.schedule_hash.clear();
        r.scheduled_at.clear();
        r.post_audience.clear();
        r.email_audience.clear();
    }
    r.updated_at = chrono::Utc::now().to_rfc3339();
    save(&r)?;
    Ok(
        json!({"receipt":r,"publication_url":c.publication_url,"message":"Gateway receipt recorded. Scheduled means accepted for release, not verified published."}),
    )
}
