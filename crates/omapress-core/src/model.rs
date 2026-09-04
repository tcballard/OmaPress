use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

pub const SCHEMA: u32 = 1;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publication {
    pub schema: u32,
    pub name: String,
    #[serde(default)]
    pub tagline: String,
    pub description: String,
    pub base_url: String,
    pub language: String,
    pub timezone: String,
    pub author: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub about: String,
    #[serde(default)]
    pub feeds: FeedConfig,
    #[serde(default)]
    pub deploy: DeployConfig,
    #[serde(default)]
    pub series: BTreeMap<String, Series>,
    #[serde(default = "default_export_order")]
    pub x_export_order: Vec<String>,
}
fn default_theme() -> String {
    "default".into()
}
fn default_export_order() -> Vec<String> {
    ["masthead", "edition", "title", "date"]
        .map(String::from)
        .to_vec()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub title: String,
    pub route: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedConfig {
    pub full_content: bool,
    pub limit: usize,
}
impl Default for FeedConfig {
    fn default() -> Self {
        Self {
            full_content: true,
            limit: 50,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeployConfig {
    #[serde(default = "provider")]
    pub provider: String,
    #[serde(default)]
    pub repository: String,
    #[serde(default = "branch")]
    pub branch: String,
}
fn provider() -> String {
    "github-pages".into()
}
fn branch() -> String {
    "gh-pages".into()
}
impl Default for DeployConfig {
    fn default() -> Self {
        Self {
            provider: provider(),
            repository: String::new(),
            branch: branch(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticleMeta {
    pub schema: u32,
    pub id: uuid::Uuid,
    #[serde(default)]
    pub title: String,
    pub series: String,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<DateTime<FixedOffset>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<FixedOffset>>,
    #[serde(default)]
    pub summary: String,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_image: Option<String>,
    #[serde(default)]
    pub x_caption: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x_url: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_window: Option<SourceWindow>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceWindow {
    pub starts_at: DateTime<FixedOffset>,
    pub ends_at: DateTime<FixedOffset>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Draft,
    Ready,
    Published,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Article {
    pub path: String,
    pub meta: ArticleMeta,
    pub body: String,
    pub hash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: String,
    pub path: String,
    pub message: String,
}
impl Diagnostic {
    pub fn error(path: &str, message: impl Into<String>) -> Self {
        Self {
            severity: "error".into(),
            path: path.into(),
            message: message.into(),
        }
    }
    pub fn warning(path: &str, message: impl Into<String>) -> Self {
        Self {
            severity: "warning".into(),
            path: path.into(),
            message: message.into(),
        }
    }
}
pub fn slug_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}
pub fn validate_config(p: &Publication) -> Result<()> {
    ensure!(
        p.schema == SCHEMA,
        "Unsupported publication schema {}; upgrade OmaPress before opening it",
        p.schema
    );
    ensure!(
        !p.name.trim().is_empty() && !p.author.trim().is_empty(),
        "Publication name and author are required"
    );
    let u = Url::parse(&p.base_url).context("Canonical base URL is invalid")?;
    ensure!(
        ["https", "http"].contains(&u.scheme())
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none()
            && u.query().is_none()
            && u.fragment().is_none(),
        "Base URL must be an HTTP(S) origin or project path without credentials, query or fragment"
    );
    ensure!(
        p.timezone.parse::<chrono_tz::Tz>().is_ok(),
        "Unknown publication timezone"
    );
    ensure!(
        p.language.len() <= 35
            && p.language
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-'),
        "Invalid language tag"
    );
    ensure!(
        (1..=1000).contains(&p.feeds.limit),
        "Feed limit must be 1–1000"
    );
    ensure!(slug_ok(&p.theme), "Invalid theme name");
    ensure!(
        p.deploy.provider == "github-pages",
        "Only github-pages is supported"
    );
    let reserved = ["media", "static", "archive", "about", "assets"];
    let mut routes = std::collections::BTreeSet::new();
    for (id, s) in &p.series {
        ensure!(
            slug_ok(id) && slug_ok(&s.route) && !s.title.trim().is_empty(),
            "Invalid series {id}"
        );
        ensure!(
            !reserved.contains(&s.route.as_str()) && routes.insert(&s.route),
            "Duplicate or reserved series route {}",
            s.route
        );
    }
    let order = &p.x_export_order;
    ensure!(
        order.len() == 4 && default_export_order().iter().all(|s| order.contains(s)),
        "X export order must contain masthead, edition, title and date once each"
    );
    Ok(())
}
pub fn parse_article(path: &str, bytes: &[u8]) -> Result<Article> {
    ensure!(bytes.len() <= 4 * 1024 * 1024, "Article exceeds 4 MiB");
    let text = std::str::from_utf8(bytes)
        .context("Article must be UTF-8")?
        .replace("\r\n", "\n");
    let rest = text
        .strip_prefix("---\n")
        .context("Article requires YAML front matter delimited by ---")?;
    let (yaml, body) = rest
        .split_once("\n---\n")
        .context("Missing closing front matter delimiter")?;
    ensure!(yaml.len() <= 65536, "Front matter exceeds 64 KiB");
    let meta: ArticleMeta = serde_yaml::from_str(yaml).context("Invalid article front matter")?;
    ensure!(
        meta.schema == SCHEMA,
        "Unsupported article schema {}",
        meta.schema
    );
    ensure!(!meta.id.is_nil(), "Article ID cannot be nil");
    ensure!(
        slug_ok(&meta.slug) && slug_ok(&meta.series),
        "Slug and series must use lowercase letters, digits and hyphens"
    );
    Ok(Article {
        path: path.into(),
        meta,
        body: body.into(),
        hash: crate::storage::hash(bytes),
    })
}
pub fn article_text(meta: &ArticleMeta, body: &str) -> Result<String> {
    Ok(format!(
        "---\n{}---\n{}",
        serde_yaml::to_string(meta)?,
        body
    ))
}
pub fn series_route<'a>(p: &'a Publication, a: &'a ArticleMeta) -> &'a str {
    p.series
        .get(&a.series)
        .map(|s| s.route.as_str())
        .unwrap_or(&a.series)
}
pub fn article_route(p: &Publication, a: &ArticleMeta) -> String {
    format!("{}/{}/", series_route(p, a), a.slug)
}
pub fn absolute(p: &Publication, route: &str) -> String {
    format!(
        "{}/{}",
        p.base_url.trim_end_matches('/'),
        route.trim_start_matches('/')
    )
}
pub fn checked_link(s: &str) -> Result<()> {
    ensure!(
        !s.chars().any(|c| c.is_control()),
        "Link contains control characters"
    );
    let u = Url::parse(s).context("Links must be complete https://, http:// or mailto: URLs")?;
    ensure!(
        ["http", "https", "mailto"].contains(&u.scheme()),
        "Unsupported link scheme: {}",
        u.scheme()
    );
    ensure!(
        u.username().is_empty() && u.password().is_none(),
        "Links cannot contain credentials"
    );
    if u.scheme() == "mailto" {
        ensure!(u.path().contains('@'), "Invalid email link");
    } else {
        ensure!(u.host_str().is_some(), "Link is missing its host");
    }
    Ok(())
}
pub fn image_path(s: &str) -> Result<Option<String>> {
    if s.starts_with("https://") || s.starts_with("http://") {
        checked_link(s)?;
        return Ok(None);
    }
    ensure!(
        s.starts_with("/media/"),
        "Local images must use /media/ paths"
    );
    let path = s.trim_start_matches('/');
    crate::storage::safe_relative(path)?;
    ensure!(
        !path.contains('%') && !path.contains('?') && !path.contains('#'),
        "Media paths cannot contain URL escapes, queries or fragments"
    );
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if !["png", "jpg", "jpeg", "webp", "gif", "avif"].contains(&ext.as_str()) {
        bail!("Unsupported image format; import PNG, JPEG, WebP, GIF or AVIF");
    }
    Ok(Some(path.into()))
}
