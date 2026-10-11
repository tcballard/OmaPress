//! Brand-independent service connection; publication receipts remain in the existing private state directory.
use crate::{model::*, process, storage::*};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Connection {
    service_url: String,
    api_token: String,
}
fn validate(c: &Connection) -> Result<()> {
    let u = url::Url::parse(&c.service_url)?;
    ensure!(
        (u.scheme() == "https"
            || (u.scheme() == "http" && matches!(u.host_str(), Some("127.0.0.1" | "[::1]"))))
            && u.host_str().is_some()
            && u.path() == "/"
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none(),
        "Use an HTTPS service origin or numeric loopback HTTP for development"
    );
    ensure!(
        c.api_token.len() >= 32
            && c.api_token.len() <= 4096
            && c.api_token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)),
        "API token must contain at least 32 letters, digits, underscores or hyphens"
    );
    Ok(())
}
fn credential_file() -> Result<Option<PathBuf>> {
    use std::os::unix::fs::MetadataExt;
    let Some(p) = std::env::var_os("NEWSLETTER_CREDENTIALS_FILE") else {
        return Ok(None);
    };
    let p = PathBuf::from(p);
    ensure!(p.is_absolute(), "Credential file must be absolute");
    let m = std::fs::symlink_metadata(&p)?;
    ensure!(
        m.is_file() && m.mode() & 0o077 == 0 && m.uid() == unsafe { libc::geteuid() },
        "Credential file must be an owner-only regular file"
    );
    Ok(Some(p))
}
fn read() -> Result<Connection> {
    let bytes = if let Some(p) = credential_file()? {
        read_bounded(&p, 8192)?
    } else {
        process::run(
            "secret-tool",
            &[
                "lookup",
                "application",
                "publishing",
                "service",
                "newsletter-v1",
            ],
            None,
            Duration::from_secs(15),
        )
        .map_err(|_| anyhow::anyhow!("Connect a newsletter service first"))?
        .into_bytes()
    };
    let c = serde_json::from_slice(&bytes).context("Invalid newsletter connection")?;
    validate(&c)?;
    Ok(c)
}
fn request(
    c: &Connection,
    method: &str,
    path: &str,
    payload: Option<(&[u8], &str)>,
) -> Result<Value> {
    let mut cfg = tempfile::NamedTempFile::new()?;
    writeln!(cfg, "header = \"Authorization: Bearer {}\"", c.api_token)?;
    let mut body = tempfile::NamedTempFile::new()?;
    if let Some((bytes, _)) = payload {
        body.write_all(bytes)?;
    }
    let endpoint = format!("{}/v1/{path}", c.service_url.trim_end_matches('/'));
    let data = format!("@{}", body.path().display());
    let content = format!(
        "Content-Type: {}",
        payload.map(|p| p.1).unwrap_or("application/json")
    );
    let mut args = vec![
        "--disable",
        "--silent",
        "--max-time",
        "60",
        "--max-filesize",
        "12582912",
        "--proto",
        "=http,https",
        "--config",
        cfg.path().to_str().context("Temporary path")?,
        "--request",
        method,
        "--write-out",
        "\n%{http_code}",
    ];
    if payload.is_some() {
        args.extend(["--header", &content, "--data-binary", &data]);
    }
    args.push(&endpoint);
    let out=process::run("curl",&args,None,Duration::from_secs(65)).map_err(|_|anyhow::anyhow!("Newsletter request interrupted. Refresh the campaign before retrying; its identity is preserved."))?;
    let (body, status) = out.rsplit_once('\n').context("Invalid service response")?;
    let value: Value = serde_json::from_str(body).context("Invalid service response")?;
    ensure!(
        status.starts_with('2'),
        "Newsletter service: {}",
        value["error"].as_str().unwrap_or("Request failed")
    );
    Ok(value)
}
fn json_request(c: &Connection, method: &str, path: &str, body: &Value) -> Result<Value> {
    request(
        c,
        method,
        path,
        Some((&serde_json::to_vec(body)?, "application/json")),
    )
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    schema: u32,
    entries: BTreeMap<String, Value>,
}
fn load(root: &Path) -> Result<Ledger> {
    let p = safe_path(root, ".omapress/newsletter.json")?;
    if !p.exists() {
        return Ok(Ledger {
            schema: 1,
            ..Default::default()
        });
    }
    let l: Ledger = serde_json::from_slice(&read_bounded(&p, 8 * 1024 * 1024)?)
        .context("Newsletter history is corrupt; restore it before continuing")?;
    ensure!(l.schema == 1, "Unsupported newsletter history");
    Ok(l)
}
fn save(root: &Path, l: &Ledger) -> Result<()> {
    atomic_write(
        &safe_path(root, ".omapress/newsletter.json")?,
        &serde_json::to_vec_pretty(l)?,
    )
}
fn image(c: &Connection, s: &Snapshot, path: &str) -> Result<String> {
    if path.starts_with("https://") {
        return Ok(path.into());
    }
    let path = path.trim_start_matches('/');
    ensure!(
        path.starts_with("media/"),
        "Import newsletter artwork into the publication first"
    );
    let bytes = s.files.get(path).context("Image is missing")?;
    let (_, mime) = detect_image(bytes)?;
    ensure!(
        ["image/png", "image/jpeg"].contains(&mime) && bytes.len() <= 5 * 1024 * 1024,
        "Newsletter artwork must be PNG/JPEG up to 5 MiB"
    );
    let v = request(c, "POST", "assets", Some((bytes, mime)))?;
    let u = v["url"].as_str().context("Service returned no image URL")?;
    let parsed = url::Url::parse(u)?;
    ensure!(
        parsed.scheme() == "https" && parsed.origin() == url::Url::parse(&c.service_url)?.origin(),
        "Service artwork requires HTTPS on the configured origin"
    );
    Ok(u.into())
}
fn body(c: &Connection, s: &Snapshot, a: &Article) -> Result<String> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    let mut edits = vec![];
    for (event, range) in Parser::new_ext(&a.body, Options::all()).into_offset_iter() {
        match event {
            Event::Html(_) | Event::InlineHtml(_) => {
                anyhow::bail!("Raw HTML is not supported in newsletter articles")
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let original = &a.body[range.clone()];
                let end = original
                    .find("](")
                    .context("Use inline Markdown images for newsletter export")?;
                let url = image(c, s, &dest_url)?;
                edits.push((range, format!("![{}]({url})", &original[2..end])));
            }
            _ => {}
        }
    }
    let mut body = a.body.clone();
    for (range, text) in edits.into_iter().rev() {
        body.replace_range(range, &text);
    }
    if let Some(cover) = &a.meta.header_image {
        body = format!("![Cover image]({})\n\n{body}", image(c, s, cover)?);
    }
    Ok(body)
}
pub fn action(root: &Path, command: &str, args: &Value) -> Result<Value> {
    if command == "newsletter-connect" {
        let c: Connection = serde_json::from_value(args.clone())?;
        validate(&c)?;
        let mut result = request(&c, "GET", "newsletters", None)?;
        result["service_url"] = json!(c.service_url);
        let bytes = serde_json::to_vec(&c)?;
        if let Some(p) = credential_file()? {
            atomic_write(&p, &bytes)?;
        } else {
            process::run_input(
                "secret-tool",
                &[
                    "store",
                    "--label=Newsletter service",
                    "application",
                    "publishing",
                    "service",
                    "newsletter-v1",
                ],
                None,
                Duration::from_secs(15),
                &bytes,
            )
            .map_err(|_| {
                anyhow::anyhow!("Cannot store newsletter credentials in Secret Service")
            })?;
        }
        return Ok(result);
    }
    let c = read()?;
    if command == "newsletter-list" {
        let mut result = request(&c, "GET", "newsletters", None)?;
        result["service_url"] = json!(c.service_url);
        return Ok(result);
    }
    if command == "newsletter-create" {
        return json_request(&c, "POST", "newsletters", args);
    }
    let newsletter = Uuid::parse_str(
        args["newsletter_id"]
            .as_str()
            .context("Select a newsletter")?,
    )?;
    if command == "newsletter-contacts" {
        return request(
            &c,
            "GET",
            &format!("newsletters/{newsletter}/contacts"),
            None,
        );
    }
    if command == "newsletter-import" {
        let mut p = args.clone();
        p.as_object_mut().unwrap().remove("newsletter_id");
        return json_request(
            &c,
            "POST",
            &format!("newsletters/{newsletter}/contacts"),
            &p,
        );
    }
    let _lock = lock(root)?;
    let s = snapshot(root)?;
    let path = args["article"].as_str().context("Select an article")?;
    let a = parse_article(path, s.files.get(path).context("Article missing")?)?;
    let key = format!(
        "{}:{newsletter}:{}",
        hash(c.service_url.trim_end_matches('/').as_bytes()),
        a.meta.id
    );
    let mut ledger = load(root)?;
    if command == "newsletter-prepare" {
        s.expected(
            args["expected_source_hash"]
                .as_str()
                .context("Review the saved article")?,
        )?;
        ensure!(
            a.meta.status != Status::Draft,
            "Mark the saved article Ready before preparing a newsletter"
        );
        let payload = json!({"newsletter_id":newsletter,"external_id":a.meta.id,"subject":a.meta.title,"preview_text":a.meta.summary,"markdown":body(&c,&s,&a)?});
        let result = json_request(&c, "POST", "campaigns", &payload)?;
        let id = Uuid::parse_str(
            result["campaign"]["id"]
                .as_str()
                .context("Service returned no campaign ID")?,
        )?;
        ledger.entries.insert(key.clone(),json!({"campaign_id":id,"article_hash":a.hash,"revision":result["campaign"]["revision"]}));
        save(root, &ledger)?;
    }
    let receipt = ledger
        .entries
        .get_mut(&key)
        .context("Prepare this article for the selected newsletter first")?;
    let id = Uuid::parse_str(
        receipt["campaign_id"]
            .as_str()
            .context("Invalid campaign receipt")?,
    )?;
    let endpoint = format!("campaigns/{id}");
    match command {
        "newsletter-prepare" | "newsletter-review" => {
            request(&c, "GET", &format!("{endpoint}/review"), None)
        }
        "newsletter-refresh" => request(&c, "GET", &endpoint, None),
        "newsletter-cancel" => json_request(&c, "POST", &format!("{endpoint}/cancel"), &json!({})),
        "newsletter-test" | "newsletter-schedule" => {
            s.expected(
                args["expected_source_hash"]
                    .as_str()
                    .context("Review the saved article")?,
            )?;
            ensure!(
                receipt["article_hash"] == a.hash,
                "Article changed; prepare and review the saved version again"
            );
            ensure!(
                args["revision"] == receipt["revision"],
                "Review this campaign revision before sending"
            );
            if command == "newsletter-test" {
                let request_id = Uuid::parse_str(
                    args["request_id"]
                        .as_str()
                        .context("Missing test request ID")?,
                )?;
                return json_request(
                    &c,
                    "POST",
                    &format!("{endpoint}/test"),
                    &json!({"request_id":request_id,"revision":args["revision"]}),
                );
            }
            ensure!(
                args["reviewed"] == true,
                "Review the email and audience before scheduling"
            );
            let at = if args["send_now"] == true {
                receipt["pending_schedule"]["scheduled_at"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| {
                        chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
                    })
            } else {
                crate::schedule::parse_time(args)?
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            };
            let payload = json!({"revision":args["revision"],"audience_hash":args["audience_hash"],"scheduled_at":at});
            // Persist the exact timestamp before dispatch so Send now retries stay idempotent.
            receipt["pending_schedule"] = payload.clone();
            save(root, &ledger)?;
            json_request(&c, "POST", &format!("{endpoint}/schedule"), &payload)
        }
        _ => anyhow::bail!("Unknown newsletter action"),
    }
}
