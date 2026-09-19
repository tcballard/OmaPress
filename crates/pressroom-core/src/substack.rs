//! A local, explicitly prepared outbox. The browser companion never receives credentials.
use crate::{model::*, render, storage::*, x_article::base64};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
};
pub const ORIGIN: &str = include_str!("../../../companion/origin.txt");
pub(crate) fn directory() -> Result<PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state")
        });
    ensure!(state.is_absolute(), "State home must be absolute");
    std::fs::create_dir_all(&state)?;
    let path = safe_path(&state, "pressroom")?;
    std::fs::create_dir_all(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    Ok(path)
}
fn registry(dir: &std::path::Path) -> Result<Value> {
    let p = safe_path(dir, "outbox.json")?;
    if !p.exists() {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_slice(&read_bounded(&p, 1024 * 1024)?)?;
    ensure!(value.is_object(), "Invalid browser outbox");
    Ok(value)
}
pub fn prepare(root: &std::path::Path, article: &str, expected: &str) -> Result<Value> {
    let _publication = lock(root)?;
    let s = snapshot(root)?;
    s.expected(expected)?;
    let a = parse_article(article, s.files.get(article).context("Article missing")?)?;
    ensure!(
        a.meta.status != Status::Draft,
        "Mark the article Ready first"
    );
    ensure!(
        !render::diagnostics(&s, &state(root)?)
            .iter()
            .any(|d| d.severity == "error"),
        "Resolve publication checks first"
    );
    let mut html = render::markdown(&a.body, &s.config, false);
    if let Some(cover) = &a.meta.header_image {
        let url = if cover.starts_with('/') {
            format!("{}{}", s.config.base_url.trim_end_matches('/'), cover)
        } else {
            cover.clone()
        };
        html = format!(
            "<p><img src=\"{}\" alt=\"Cover image\"></p>{html}",
            url.replace('&', "&amp;").replace('"', "&quot;")
        );
    }
    // Embed only local assets actually referenced by the generated HTML.
    for (path, bytes) in &s.files {
        if !path.starts_with("media/") {
            continue;
        }
        let url = format!("{}/{}", s.config.base_url.trim_end_matches('/'), path);
        let escaped = url.replace('&', "&amp;").replace('"', "&quot;");
        if !html.contains(&format!("src=\"{escaped}\"")) {
            continue;
        }
        ensure!(
            bytes.len() <= 5 * 1024 * 1024,
            "Substack image exceeds 5 MiB"
        );
        let mime = if bytes.starts_with(b"\x89PNG") {
            "image/png"
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            "image/jpeg"
        } else {
            anyhow::bail!("Substack companion accepts PNG/JPEG images")
        };
        html = html.replace(
            &format!("src=\"{escaped}\""),
            &format!("src=\"data:{mime};base64,{}\"", base64(bytes)),
        );
    }
    ensure!(
        !html.contains("src=\"https:") && !html.contains("src=\"http:"),
        "Import remote images locally before preparing Substack"
    );
    let id = hash(format!("{}:{}:{}", s.root.display(), a.meta.id, a.hash).as_bytes());
    let bytes = serde_json::to_vec(
        &json!({"schema":1,"id":id,"title":a.meta.title,"subtitle":a.meta.summary,"html":html,"article_hash":a.hash}),
    )?;
    ensure!(
        bytes.len() <= 32 * 1024 * 1024,
        "Browser export exceeds 32 MiB"
    );
    crate::distribution::substack_prepared(root, &a, &id)?;
    let dir = directory()?;
    let _outbox = lock(&dir)?;
    let mut entries = registry(&dir)?;
    ensure!(
        entries.as_object().unwrap().len() < 100 || entries.get(&id).is_some(),
        "Outbox is full; clear completed entries in the browser companion"
    );
    atomic_write(&safe_path(&dir, &format!("{id}.json"))?, &bytes)?;
    entries[&id] = json!({"id":id,"title":a.meta.title,"root":s.root,"article":article,"source_hash":expected,"bundle_hash":hash(&bytes),"size":bytes.len()});
    atomic_write(
        &safe_path(&dir, "outbox.json")?,
        &serde_json::to_vec(&entries)?,
    )?;
    Ok(
        json!({"status":"awaiting_browser","outbox_id":id,"title":a.meta.title,"message":"Open a blank Substack draft, then use the Pressroom browser companion. Review audience and email settings in Substack before publishing."}),
    )
}
pub fn message(request: &Value) -> Result<Value> {
    let dir = directory()?;
    let entries = registry(&dir)?;
    let command = request["command"]
        .as_str()
        .context("Missing companion command")?;
    if command == "list" {
        return Ok(
            json!({"items":entries.as_object().unwrap().values().map(|e|json!({"id":e["id"],"title":e["title"]})).collect::<Vec<_>>()}),
        );
    }
    let id = request["id"].as_str().context("Missing outbox ID")?;
    ensure!(
        id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid outbox ID"
    );
    let entry = entries.get(id).context("Outbox entry no longer exists")?;
    if command == "remove" {
        let _lock = lock(&dir)?;
        let mut current = registry(&dir)?;
        current.as_object_mut().unwrap().remove(id);
        atomic_write(
            &safe_path(&dir, "outbox.json")?,
            &serde_json::to_vec(&current)?,
        )?;
        let path = safe_path(&dir, &format!("{id}.json"))?;
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        return Ok(json!({"removed":true}));
    }
    let root = PathBuf::from(entry["root"].as_str().context("Invalid outbox root")?);
    let expected = entry["source_hash"]
        .as_str()
        .context("Invalid outbox hash")?;
    snapshot(&root)?.expected(expected)?;
    match command {
        "read" => {
            let bytes = read_bounded(&safe_path(&dir, &format!("{id}.json"))?, 32 * 1024 * 1024)?;
            ensure!(
                hash(&bytes) == entry["bundle_hash"],
                "Outbox content changed; prepare it again"
            );
            let offset = request["offset"].as_u64().unwrap_or(0) as usize;
            ensure!(offset <= bytes.len(), "Invalid outbox offset");
            let end = (offset + 128 * 1024).min(bytes.len());
            Ok(
                json!({"data":base64(&bytes[offset..end]),"next":end,"total":bytes.len(),"hash":entry["bundle_hash"]}),
            )
        }
        "confirm" => {
            crate::distribution::confirm(
                &root,
                entry["article"].as_str().context("Invalid article")?,
                expected,
                "substack",
                request["url"].as_str().context("Missing public URL")?,
            )?;
            Ok(json!({"confirmed_by_user":true}))
        }
        _ => anyhow::bail!("Unsupported companion command"),
    }
}
pub fn native(origin: &str) -> Result<()> {
    ensure!(origin == ORIGIN.trim(), "Unknown browser companion origin");
    let mut length = [0; 4];
    std::io::stdin().read_exact(&mut length)?;
    let length = u32::from_ne_bytes(length) as usize;
    ensure!(length <= 8192, "Native request too large");
    let mut input = vec![0; length];
    std::io::stdin().read_exact(&mut input)?;
    let result = serde_json::from_slice(&input)
        .map_err(anyhow::Error::from)
        .and_then(|v| message(&v));
    let response = crate::protocol::response(result);
    let bytes = serde_json::to_vec(&response)?;
    ensure!(bytes.len() < 1024 * 1024, "Native response too large");
    std::io::stdout().write_all(&(bytes.len() as u32).to_ne_bytes())?;
    std::io::stdout().write_all(&bytes)?;
    std::io::stdout().flush()?;
    Ok(())
}
