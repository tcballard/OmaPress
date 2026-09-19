//! Official X Articles API adapter. Credentials remain in the desktop secret service.
use crate::{model::Article, process};
use anyhow::{Result, ensure};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde_json::{Value, json};
use std::{io::Write, time::Duration};

/// Conservative text-only subset until rich entity/media mapping is live-verified.
/// Reject unsupported content rather than silently flattening a reviewed article.
pub fn payload(a: &Article) -> Result<Value> {
    ensure!(
        a.meta.header_image.is_none(),
        "X API image upload is not validated yet. Use the rich-copy export for articles with artwork."
    );
    let mut blocks = Vec::new();
    let mut text = String::new();
    for event in Parser::new_ext(&a.body, Options::all()) {
        match event {
            Event::Text(t) => text.push_str(&t),
            Event::SoftBreak | Event::HardBreak => text.push('\n'),
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                blocks
                    .push(json!({"key":format!("b{}",blocks.len()),"text":text,"type":"unstyled"}));
                text = String::new();
            }
            _ => anyhow::bail!(
                "This article uses formatting not yet validated with the X API. Use rich-copy export to preserve it."
            ),
        }
    }
    ensure!(!blocks.is_empty(), "Article body is empty");
    Ok(json!({"title":a.meta.title,"content_state":{"blocks":blocks,"entities":[]}}))
}
pub fn token() -> Result<String> {
    let value = crate::connections::access_token()?;
    let value = value.trim().to_string();
    ensure!(
        !value.is_empty()
            && value.len() < 16384
            && value
                .bytes()
                .all(|b| b.is_ascii_graphic() && b != b'\"' && b != b'\\'),
        "No valid X OAuth token in Secret Service"
    );
    Ok(value)
}
pub fn request(token: &str, path: &str, body: &Value) -> Result<Value> {
    ensure!(
        path == "/2/articles/draft"
            || (path.starts_with("/2/articles/") && path.ends_with("/publish")),
        "Unsupported X operation"
    );
    // Private tempfiles keep token and article body out of process arguments.
    let mut config = tempfile::NamedTempFile::new()?;
    writeln!(config, "header = \"Authorization: Bearer {token}\"")?;
    let mut payload = tempfile::NamedTempFile::new()?;
    payload.write_all(&serde_json::to_vec(body)?)?;
    let url = format!("https://api.x.com{path}");
    let data = format!("@{}", payload.path().display());
    let output=process::run("curl",&["--disable","--silent","--show-error","--fail","--max-time","60","--proto","=https","--config",config.path().to_str().unwrap(),"--header","Content-Type: application/json","--data-binary",&data,&url],None,Duration::from_secs(65))
        .map_err(|_|anyhow::anyhow!("X request did not complete successfully. Its outcome may be unknown; check X before retrying."))?;
    let value: Value = serde_json::from_str(&output)?;
    ensure!(
        value.get("errors").is_none(),
        "X returned an error; check X before retrying"
    );
    Ok(value)
}
