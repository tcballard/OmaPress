use crate::{deploy, model::*, render, storage::*};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: u32,
    pub command: String,
    #[serde(default)]
    pub path: PathBuf,
    #[serde(default)]
    pub args: Value,
}
#[derive(Serialize)]
pub struct Response {
    pub schema: u32,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
pub fn response(result: Result<Value>) -> Response {
    match result {
        Ok(v) => Response {
            schema: SCHEMA,
            ok: true,
            result: Some(v),
            error: None,
        },
        Err(e) => Response {
            schema: SCHEMA,
            ok: false,
            result: None,
            error: Some(format!("{e:#}")),
        },
    }
}
fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .with_context(|| format!("Missing string argument: {key}"))
}
pub fn inspect(root: &Path) -> Result<Value> {
    let s = snapshot(root)?;
    let ledger = state(root)?;
    let (articles, parse_errors) = s.articles();
    let mut issues = render::diagnostics(&s, &ledger);
    for e in parse_errors {
        if !issues
            .iter()
            .any(|d| d.path == e.path && d.message == e.message)
        {
            issues.push(e);
        }
    }
    let latest = ledger.deployments.last();
    let articles:Vec<_>=articles.iter().map(|a|{let published=latest.is_some_and(|d|d.articles.get(&a.meta.id.to_string())==Some(&a.hash));json!({"path":a.path,"meta":a.meta,"hash":a.hash,"search":format!("{} {} {} {}",a.meta.title,a.meta.summary,a.body,a.meta.tags.join(" ")),"display_status":if published{"published"}else if a.meta.status==Status::Draft{"draft"}else{"ready"}})}).collect();
    Ok(
        json!({"source_hash":s.hash,"config":s.config,"config_text":String::from_utf8(s.files["publication.toml"].clone())?,"articles":articles,"diagnostics":issues,"state":ledger}),
    )
}
pub fn execute(req: Request) -> Result<Value> {
    ensure!(
        req.schema == SCHEMA,
        "Unsupported protocol schema; update desktop and CLI together"
    );
    let root = &req.path;
    let a = &req.args;
    match req.command.as_str() {
        "init" => {
            let s = init(
                root,
                string(a, "name")?,
                string(a, "base_url")?,
                string(a, "author")?,
            )?;
            inspect(&s.root)
        }
        "inspect" => inspect(root),
        "validate" => {
            let s = snapshot(root)?;
            let d = render::diagnostics(&s, &state(root)?);
            Ok(
                json!({"source_hash":s.hash,"valid":!d.iter().any(|i|i.severity=="error"),"diagnostics":d}),
            )
        }
        "read" => {
            let s = snapshot(root)?;
            let path = string(a, "article")?;
            let data = s.files.get(path).context("Article not found")?;
            let article = parse_article(path, data)?;
            Ok(
                json!({"source_hash":s.hash,"article":article,"text":String::from_utf8(data.clone())?,"html":render::markdown(&article.body,&s.config,true),"recovery":recovery_read(root,path)?}),
            )
        }
        "render" => {
            let s = snapshot(root)?;
            let text = string(a, "text")?;
            let article = parse_article("content/preview.md", text.as_bytes())?;
            let mut html = render::markdown(&article.body, &s.config, true);
            // TextEdit's rich-text image loader receives local files only.
            html = html.replace(
                &format!("{}/media/", s.config.base_url.trim_end_matches('/')),
                &format!("file://{}/media/", s.root.display()),
            );
            Ok(json!({"html":html,"export":render::export_x(&s,&article)}))
        }
        "save-document" => {
            let meta: ArticleMeta = serde_json::from_value(a["meta"].clone())?;
            let text = article_text(&meta, string(a, "body")?)?;
            save_article(
                root,
                string(a, "article")?,
                &text,
                string(a, "expected_source_hash")?,
            )?;
            inspect(root)
        }
        "render-document" => {
            let s = snapshot(root)?;
            let meta: ArticleMeta = serde_json::from_value(a["meta"].clone())?;
            let body = string(a, "body")?.to_string();
            let article = Article {
                path: "content/preview.md".into(),
                hash: String::new(),
                meta,
                body,
            };
            let html = render::markdown(&article.body, &s.config, true).replace(
                &format!("{}/media/", s.config.base_url.trim_end_matches('/')),
                &format!("file://{}/media/", s.root.display()),
            );
            Ok(json!({"html":html,"export":render::export_x(&s,&article)}))
        }
        "recovery-document" => {
            let _lock = lock(root)?;
            let path = string(a, "article")?;
            safe_relative(path)?;
            ensure!(
                path.starts_with("content/") && path.ends_with(".md"),
                "Invalid recovery path"
            );
            let bytes = serde_json::to_vec(
                &json!({"schema":SCHEMA,"article":path,"meta":a["meta"],"body":a["body"],"base_source_hash":a["expected_source_hash"],"saved_at":chrono::Utc::now().to_rfc3339()}),
            )?;
            ensure!(bytes.len() <= 5 * 1024 * 1024, "Recovery exceeds 5 MiB");
            let key = hash(path.as_bytes());
            atomic_write(
                &safe_path(root, &format!(".omapress/recovery/{key}.json"))?,
                &bytes,
            )?;
            Ok(json!({"saved":true}))
        }
        "save" => {
            save_article(
                root,
                string(a, "article")?,
                string(a, "text")?,
                string(a, "expected_source_hash")?,
            )?;
            inspect(root)
        }
        "new" => {
            let s = snapshot(root)?;
            s.expected(string(a, "expected_source_hash")?)?;
            let series = string(a, "series")?;
            ensure!(
                s.config.series.contains_key(series),
                "Select a configured series"
            );
            let id = uuid::Uuid::new_v4();
            let slug = format!("untitled-{}", &id.simple().to_string()[..8]);
            let meta = ArticleMeta {
                schema: SCHEMA,
                id,
                title: String::new(),
                series: series.into(),
                status: Status::Draft,
                published_at: Some(
                    chrono::Utc::now()
                        .with_timezone(&s.config.timezone.parse::<chrono_tz::Tz>()?)
                        .fixed_offset(),
                ),
                updated_at: None,
                summary: String::new(),
                slug: slug.clone(),
                header_image: None,
                x_caption: String::new(),
                x_url: None,
                tags: Vec::new(),
                source_window: None,
            };
            let path = format!("content/{series}/{slug}.md");
            save_article(root, &path, &article_text(&meta, "")?, &s.hash)?;
            Ok(json!({"article":path,"publication":inspect(root)?}))
        }
        "save-config" => {
            save_config(root, string(a, "text")?, string(a, "expected_source_hash")?)?;
            inspect(root)
        }
        "delete" => {
            delete_article(
                root,
                string(a, "article")?,
                string(a, "expected_source_hash")?,
            )?;
            inspect(root)
        }
        "import-media" => {
            let (s, path) = import_media(
                root,
                Path::new(string(a, "source")?),
                string(a, "expected_source_hash")?,
            )?;
            Ok(json!({"source_hash":s.hash,"media_path":path}))
        }
        "import-article" => {
            let s = snapshot(root)?;
            s.expected(string(a, "expected_source_hash")?)?;
            let bytes = read_bounded(Path::new(string(a, "source")?), 4 * 1024 * 1024)?;
            let article = parse_article("import.md", &bytes)?;
            let path = format!("content/{}/{}.md", article.meta.series, article.meta.slug);
            ensure!(
                !s.files.contains_key(&path),
                "An article already uses this path"
            );
            ensure!(
                !s.articles().0.iter().any(|x| x.meta.id == article.meta.id),
                "Article ID already exists"
            );
            save_article(root, &path, std::str::from_utf8(&bytes)?, &s.hash)?;
            Ok(json!({"article":path,"publication":inspect(root)?}))
        }
        "build" => {
            let _lock = lock(root)?;
            let s = snapshot(root)?;
            s.expected(string(a, "expected_source_hash")?)?;
            let b = render::build(&s, &state(root)?, false)?;
            snapshot(root)?.expected(&s.hash)?;
            let out = a["output"].as_str().unwrap_or("output");
            render::write_build(root, Path::new(out), &b)?;
            Ok(serde_json::to_value(&b.manifest)?)
        }
        "feed-check" => {
            let dir = safe_path(root, "output")?;
            let manifest: render::Manifest = serde_json::from_slice(&read_bounded(
                &safe_path(&dir, "build-manifest.json")?,
                8 * 1024 * 1024,
            )?)?;
            ensure!(
                !manifest.private_preview,
                "Preview feeds are not public deployment artifacts"
            );
            let mut files = std::collections::BTreeMap::new();
            for (p, h) in &manifest.files {
                let b = read_bounded(&safe_path(&dir, p)?, 32 * 1024 * 1024)?;
                ensure!(&hash(&b) == h, "Output hash mismatch: {p}");
                files.insert(p.clone(), b);
            }
            ensure!(
                tree_hash(&files) == manifest.output_hash,
                "Output tree hash mismatch"
            );
            render::feed_check_files(&files)?;
            Ok(json!({"valid":true,"feeds":manifest.feeds}))
        }
        "export-x" => {
            let s = snapshot(root)?;
            let path = string(a, "article")?;
            let article = parse_article(path, s.files.get(path).context("Article not found")?)?;
            Ok(serde_json::to_value(render::export_x(&s, &article))?)
        }
        "github-status" => deploy::github_status(),
        "repositories" => deploy::repositories(),
        "setup" => deploy::setup(
            root,
            string(a, "expected_source_hash")?,
            string(a, "repository")?,
            a["create"].as_bool().unwrap_or(false),
        ),
        "publish-plan" => deploy::plan(root, string(a, "expected_source_hash")?),
        "publish" => deploy::publish(
            root,
            string(a, "expected_source_hash")?,
            string(a, "expected_remote_head")?,
        ),
        "recheck" => deploy::recheck(root),
        "rollback" => deploy::rollback(
            root,
            string(a, "deployment_id")?,
            string(a, "expected_source_hash")?,
            string(a, "expected_remote_head")?,
        ),
        "recovery-save" => {
            let _lock = lock(root)?;
            let path = string(a, "article")?;
            safe_relative(path)?;
            ensure!(
                path.starts_with("content/") && path.ends_with(".md"),
                "Invalid recovery article path"
            );
            let text = string(a, "text")?;
            ensure!(text.len() <= 4 * 1024 * 1024, "Recovery exceeds 4 MiB");
            let key = hash(path.as_bytes());
            let data = json!({"schema":SCHEMA,"article":path,"text":text,"base_source_hash":string(a,"expected_source_hash")?,"saved_at":chrono::Utc::now().to_rfc3339()});
            atomic_write(
                &safe_path(root, &format!(".omapress/recovery/{key}.json"))?,
                &serde_json::to_vec(&data)?,
            )?;
            Ok(json!({"saved":true}))
        }
        "recovery-clear" => {
            let _lock = lock(root)?;
            let key = hash(string(a, "article")?.as_bytes());
            let path = safe_path(root, &format!(".omapress/recovery/{key}.json"))?;
            if path.exists() {
                std::fs::remove_file(path)?;
            }
            Ok(json!({"cleared":true}))
        }
        _ => bail!("Unknown command: {}", req.command),
    }
}
fn recovery_read(root: &Path, article: &str) -> Result<Value> {
    let key = hash(article.as_bytes());
    let path = safe_path(root, &format!(".omapress/recovery/{key}.json"))?;
    if !path.exists() {
        return Ok(Value::Null);
    }
    Ok(serde_json::from_slice(&read_bounded(
        &path,
        5 * 1024 * 1024,
    )?)?)
}
