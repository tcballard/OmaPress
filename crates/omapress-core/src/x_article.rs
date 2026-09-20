//! Official X Articles API adapter. Credentials remain in the desktop secret service.
use crate::{model::Article, process};
use anyhow::{Context, Result, ensure};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde_json::{Value, json};
use std::{io::Write, time::Duration};

/// Deterministic DraftJS conversion. Offsets are UTF-16 code units, as in the web editor.
/// Constructs with no lossless mapping fail before any remote request.
pub fn payload(a: &Article) -> Result<Value> {
    let mut builder = Builder::default();
    let mut quote = false;
    let mut styles: Vec<(&str, usize)> = Vec::new();
    let mut link: Option<(usize, String)> = None;
    let mut markdown: Option<(usize, TagEnd, usize)> = None;
    let mut image: Option<(String, String)> = None;
    for (event, range) in Parser::new_ext(&a.body, Options::all()).into_offset_iter() {
        if let Some((start, end, depth)) = markdown.as_mut() {
            ensure!(
                !matches!(
                    event,
                    Event::Start(Tag::Image { .. }) | Event::Html(_) | Event::InlineHtml(_)
                ),
                "Images or HTML inside Markdown embeds need browser export"
            );
            if matches!(&event, Event::Start(tag) if tag.to_end()==*end) {
                *depth += 1;
            }
            if event == Event::End(*end) {
                if *depth > 0 {
                    *depth -= 1;
                } else {
                    builder.atomic(
                        "markdown",
                        json!({"markdown":&a.body[*start..range.end]}),
                        " ",
                    );
                    markdown = None;
                }
            }
            continue;
        }
        if let Some((path, alt)) = image.as_mut() {
            match event {
                Event::End(TagEnd::Image) => {
                    let data = json!({"caption":alt,"media_items":[{"media_category":"tweet_image","media_id":path}]});
                    builder.atomic("image", data, " ");
                    image = None;
                }
                Event::Text(t) | Event::Code(t) => alt.push_str(&t),
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(ref tag @ (Tag::CodeBlock(_) | Tag::Table(_) | Tag::List(_))) => {
                builder.flush();
                markdown = Some((range.start, tag.to_end(), 0));
            }
            Event::Start(Tag::Paragraph) if a.body[range.clone()].contains('`') => {
                builder.flush();
                markdown = Some((range.start, TagEnd::Paragraph, 0));
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                ensure!(
                    styles.is_empty() && link.is_none() && !quote,
                    "Images inside formatting or quotations need browser export"
                );
                let path = local_image(&dest_url)?;
                builder.flush();
                image = Some((path, String::new()));
            }
            Event::Rule => {
                builder.flush();
                builder.atomic("divider", json!({}), " ");
            }
            Event::DisplayMath(source) => {
                builder.flush();
                builder.atomic("latex", json!({}), &source);
            }

            Event::Start(Tag::Paragraph) => {
                builder.flush();
                builder.kind = if quote { "blockquote" } else { "unstyled" };
            }
            Event::Start(Tag::Heading { level, .. }) => {
                builder.flush();
                builder.kind = match level {
                    pulldown_cmark::HeadingLevel::H1 => "header-one",
                    pulldown_cmark::HeadingLevel::H2 => "header-two",
                    pulldown_cmark::HeadingLevel::H3 => "header-three",
                    _ => anyhow::bail!(
                        "X supports headings 1–3. Use a smaller heading or browser export."
                    ),
                };
            }
            Event::Start(Tag::BlockQuote(_)) => {
                ensure!(!quote, "Nested quotations need browser export");
                builder.flush();
                quote = true;
                builder.kind = "blockquote";
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                builder.flush();
                quote = false;
            }
            Event::End(
                TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item,
            ) => builder.flush(),
            Event::Start(Tag::Strong) => styles.push(("bold", builder.offset())),
            Event::Start(Tag::Emphasis) => styles.push(("italic", builder.offset())),
            Event::Start(Tag::Strikethrough) => styles.push(("strikethrough", builder.offset())),
            Event::End(TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough) => {
                let (style, offset) = styles.pop().context("Unbalanced formatting")?;
                builder
                    .styles
                    .push(json!({"style":style,"offset":offset,"length":builder.offset()-offset}));
            }
            Event::Code(_) => anyhow::bail!("Inline code outside paragraphs needs browser export"),
            Event::Start(Tag::Link { dest_url, .. }) => {
                let u =
                    url::Url::parse(&dest_url).context("X links must be absolute HTTPS URLs")?;
                ensure!(
                    matches!(u.scheme(), "https" | "http" | "mailto"),
                    "Unsafe X link"
                );
                link = Some((builder.offset(), dest_url.into_string()));
            }
            Event::End(TagEnd::Link) => {
                let (offset, url) = link.take().context("Unbalanced link")?;
                let key = builder.entities.len();
                builder.entities.push(json!({"key":key.to_string(),"value":{"type":"link","mutability":"mutable","data":{"url":url}}}));
                builder
                    .ranges
                    .push(json!({"key":key,"offset":offset,"length":builder.offset()-offset}));
            }
            Event::Text(t) => builder.text.push_str(&t),
            Event::SoftBreak | Event::HardBreak => builder.text.push('\n'),
            _ => anyhow::bail!(
                "This article contains a table, image, HTML, or other construct without a verified X mapping. Use browser export to preserve it."
            ),
        }
    }
    builder.flush();
    ensure!(!builder.blocks.is_empty(), "Article body is empty");
    let mut value = json!({"title":a.meta.title,"content_state":{"blocks":builder.blocks,"entities":builder.entities}});
    if let Some(path) = &a.meta.header_image {
        value["cover_media"] =
            json!({"media_category":"tweet_image","media_id":local_image(path)?});
    }
    Ok(value)
}
struct Builder {
    blocks: Vec<Value>,
    entities: Vec<Value>,
    text: String,
    kind: &'static str,
    styles: Vec<Value>,
    ranges: Vec<Value>,
}
impl Default for Builder {
    fn default() -> Self {
        Self {
            blocks: vec![],
            entities: vec![],
            text: String::new(),
            kind: "unstyled",
            styles: vec![],
            ranges: vec![],
        }
    }
}
impl Builder {
    fn atomic(&mut self, kind: &str, data: Value, text: &str) {
        let key = self.entities.len();
        self.entities.push(json!({"key":key.to_string(),"value":{"type":kind,"mutability":if kind=="markdown" {"mutable"}else{"immutable"},"data":data}}));
        self.blocks.push(json!({"key":format!("b{}",self.blocks.len()),"text":text,"type":"atomic","entity_ranges":[{"key":key,"offset":0,"length":text.encode_utf16().count()}]}));
    }

    fn offset(&self) -> usize {
        self.text.encode_utf16().count()
    }
    fn flush(&mut self) {
        if !self.text.is_empty() {
            self.blocks.push(json!({"key":format!("b{}",self.blocks.len()),"text":std::mem::take(&mut self.text),"type":self.kind,"inline_style_ranges":std::mem::take(&mut self.styles),"entity_ranges":std::mem::take(&mut self.ranges)}));
        }
        self.kind = "unstyled";
    }
}
pub fn token() -> Result<String> {
    Ok(credentials()?.0)
}
pub fn credentials() -> Result<(String, String)> {
    let bundle = crate::connections::credentials()?;
    let value = bundle["access_token"].as_str().context("Reconnect X")?;
    let value = value.trim().to_string();
    ensure!(
        !value.is_empty()
            && value.len() < 16384
            && value
                .bytes()
                .all(|b| b.is_ascii_graphic() && b != b'\"' && b != b'\\'),
        "No valid X OAuth token in Secret Service"
    );
    let account = bundle["user_id"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("legacy-{}", crate::storage::hash(value.as_bytes())));
    Ok((value, account))
}
pub fn request(token: &str, path: &str, body: &Value) -> Result<Value> {
    ensure!(
        path == "/2/media/upload"
            || path == "/2/articles/draft"
            || (path.starts_with("/2/articles/") && path.ends_with("/publish")),
        "Unsupported X operation"
    );
    // Private tempfiles keep token and article body out of process arguments.
    let config = format!("header = \"Authorization: Bearer {token}\"\n");
    let mut payload = tempfile::NamedTempFile::new()?;
    payload.write_all(&serde_json::to_vec(body)?)?;
    let url = format!("https://api.x.com{path}");
    let data = format!("@{}", payload.path().display());
    let output=process::run_input("curl",&["--disable","--silent","--show-error","--fail","--max-time","60","--proto","=https","--config","-","--header","Content-Type: application/json","--data-binary",&data,&url],None,Duration::from_secs(65),config.as_bytes())
        .map_err(|_|anyhow::anyhow!("X request did not complete successfully. Its outcome may be unknown; check X before retrying."))?;
    let value: Value = serde_json::from_str(&output)?;
    ensure!(
        value.get("errors").is_none(),
        "X returned an error; check X before retrying"
    );
    Ok(value)
}

fn local_image(path: &str) -> Result<String> {
    let path = path.strip_prefix('/').unwrap_or(path);
    crate::storage::safe_relative(path)?;
    ensure!(
        path.starts_with("media/")
            && (path.ends_with(".png") || path.ends_with(".jpg") || path.ends_with(".jpeg")),
        "X images must be imported local PNG or JPEG files"
    );
    Ok(path.into())
}
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((c.get(1).copied().unwrap_or(0) as u32) << 8)
            | c.get(2).copied().unwrap_or(0) as u32;
        for i in 0..4 {
            out.push(if i <= c.len() {
                TABLE[((n >> (18 - i * 6)) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}
/// Validate every referenced asset before uploading any of them.
pub fn media_paths(s: &crate::storage::Snapshot, payload: &Value) -> Result<Vec<String>> {
    let mut paths = std::collections::BTreeSet::new();
    if let Some(p) = payload["cover_media"]["media_id"].as_str() {
        paths.insert(p.to_owned());
    }
    for e in payload["content_state"]["entities"]
        .as_array()
        .context("Missing entities")?
    {
        if e["value"]["type"] == "image" {
            for item in e["value"]["data"]["media_items"]
                .as_array()
                .context("Missing media")?
            {
                paths.insert(
                    item["media_id"]
                        .as_str()
                        .context("Missing media path")?
                        .into(),
                );
            }
        }
    }
    for p in &paths {
        local_image(p)?;
        let bytes = s
            .files
            .get(p)
            .context("Image not found in saved publication")?;
        ensure!(bytes.len() <= 5 * 1024 * 1024, "X image exceeds 5 MiB");
        ensure!(
            bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(&[0xff, 0xd8, 0xff]),
            "X image is not PNG or JPEG"
        );
    }
    Ok(paths.into_iter().collect())
}
pub fn upload_media(s: &crate::storage::Snapshot, token: &str, payload: &mut Value) -> Result<()> {
    let paths = media_paths(s, payload)?;
    let mut ids = std::collections::BTreeMap::new();
    for p in paths {
        let v = request(
            token,
            "/2/media/upload",
            &json!({"media":base64(&s.files[&p]),"media_category":"tweet_image"}),
        )?;
        let id = v["data"]["id"].as_str().context("X returned no media ID")?;
        ensure!(
            !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()),
            "Invalid X media ID"
        );
        ensure!(
            v["data"]["processing_info"].is_null()
                || v["data"]["processing_info"]["state"] == "succeeded",
            "X image is still processing; check the upload before retrying"
        );
        ids.insert(p, id.to_owned());
    }
    if let Some(p) = payload["cover_media"]["media_id"].as_str() {
        let id = ids[p].clone();
        payload["cover_media"]["media_id"] = json!(id);
    }
    for e in payload["content_state"]["entities"]
        .as_array_mut()
        .context("Missing entities")?
    {
        if e["value"]["type"] == "image" {
            for item in e["value"]["data"]["media_items"]
                .as_array_mut()
                .context("Missing media")?
            {
                let id = ids[item["media_id"].as_str().context("Missing media path")?].clone();
                item["media_id"] = json!(id);
            }
        }
    }
    Ok(())
}
