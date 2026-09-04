use crate::{model::*, storage::*};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Utc};
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
}
pub fn markdown(body: &str, p: &Publication, offline: bool) -> String {
    let mut external_image = false;
    let events = Parser::new_ext(body, options()).map(|e| match e {
        Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            if checked_link(&dest_url).is_err() {
                Event::Start(Tag::Link {
                    link_type,
                    dest_url: CowStr::from("#invalid-link"),
                    title,
                    id,
                })
            } else {
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            }
        }
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            if offline && matches!(image_path(&dest_url), Ok(None)) {
                external_image = true;
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title: CowStr::from("External image; load manually"),
                    id,
                })
            } else {
                let dest = match image_path(&dest_url) {
                    Ok(Some(path)) => absolute(p, &path),
                    Ok(None) => dest_url.to_string(),
                    Err(_) => String::new(),
                };
                Event::Start(Tag::Image {
                    link_type,
                    dest_url: CowStr::from(dest),
                    title,
                    id,
                })
            }
        }
        Event::End(TagEnd::Image) if external_image => {
            external_image = false;
            Event::End(TagEnd::Link)
        }
        other => other,
    });
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}
pub fn check_identity(p: &Publication, a: &Article, state: &State) -> Result<()> {
    if let Some(old) = state.identities.get(&a.meta.id.to_string()) {
        ensure!(
            old.url == absolute(p, &article_route(p, &a.meta)),
            "Published article URL is immutable (base URL, series route and slug)"
        );
        ensure!(
            a.meta.published_at.map(|d| d.to_rfc3339()).as_deref() == Some(&old.published_at),
            "First publication date is immutable"
        );
        ensure!(
            a.meta.status != Status::Draft,
            "A published article cannot become a draft"
        );
    }
    Ok(())
}
pub fn diagnostics(s: &Snapshot, ledger: &State) -> Vec<Diagnostic> {
    let (articles, mut issues) = s.articles();
    let mut ids = BTreeSet::new();
    let mut routes = BTreeSet::new();
    for a in &articles {
        let m = &a.meta;
        let path = &a.path;
        if !ids.insert(m.id.to_string()) {
            issues.push(Diagnostic::error(path, "Duplicate immutable article ID"));
        }
        if !routes.insert(article_route(&s.config, m)) {
            issues.push(Diagnostic::error(path, "Duplicate article route"));
        }
        if !s.config.series.contains_key(&m.series) {
            issues.push(Diagnostic::error(path, "Series is not configured"));
        }
        if m.title.trim().is_empty() {
            issues.push(Diagnostic::error(path, "Editorial title is required"));
        }
        if m.summary.trim().is_empty() {
            issues.push(Diagnostic::error(path, "Summary is required"));
        }
        if m.published_at.is_none() {
            issues.push(Diagnostic::error(
                path,
                "Publication date with timezone is required",
            ));
        }
        if m.published_at.is_some_and(|d| d > Utc::now()) && m.status != Status::Draft {
            issues.push(Diagnostic::error(
                path,
                "Future-dated articles must remain drafts; scheduled publishing is not enabled",
            ));
        }
        if let (Some(p), Some(u)) = (m.published_at, m.updated_at)
            && u < p
        {
            issues.push(Diagnostic::error(
                path,
                "Update date precedes publication date",
            ));
        }
        if let Some(w) = &m.source_window
            && w.ends_at <= w.starts_at
        {
            issues.push(Diagnostic::error(
                path,
                "Source window must end after it starts",
            ));
        }
        if let Err(e) = check_identity(&s.config, a, ledger) {
            issues.push(Diagnostic::error(path, e.to_string()));
        }
        if let Some(x) = &m.x_url
            && checked_link(x).is_err()
        {
            issues.push(Diagnostic::error(path, "Invalid X URL"));
        }
        let mut images = Vec::new();
        if let Some(img) = &m.header_image {
            images.push(img.to_string());
        }
        for e in Parser::new_ext(&a.body, options()) {
            match e {
                Event::Start(Tag::Link { dest_url, .. }) => {
                    if let Err(e) = checked_link(&dest_url) {
                        issues.push(Diagnostic::error(
                            path,
                            format!("Invalid source link {dest_url}: {e}"),
                        ));
                    }
                }
                Event::Start(Tag::Image { dest_url, .. }) => images.push(dest_url.to_string()),
                Event::Html(_) | Event::InlineHtml(_) => {
                    issues.push(Diagnostic::warning(path, "Raw HTML is displayed as text"))
                }
                _ => {}
            }
        }
        for img in images {
            match image_path(&img) {
                Ok(Some(p)) => match s.files.get(&p) {
                    Some(data) => {
                        if detect_image(data).is_err() {
                            issues.push(Diagnostic::error(
                                path,
                                format!("Invalid image contents: {p}"),
                            ));
                        }
                    }
                    None => issues.push(Diagnostic::error(path, format!("Missing image: {p}"))),
                },
                Ok(None) => issues.push(Diagnostic::warning(
                    path,
                    format!("External image: {img}. Import it for offline use."),
                )),
                Err(e) => issues.push(Diagnostic::error(path, format!("Invalid image: {e}"))),
            }
        }
    }
    for id in ledger.identities.keys() {
        if !ids.contains(id) {
            issues.push(Diagnostic::error(
                "publication.toml",
                format!("Previously published article {id} is missing; restore its source"),
            ));
        }
    }
    issues
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub source_hash: String,
    pub output_hash: String,
    pub private_preview: bool,
    pub files: BTreeMap<String, String>,
    pub articles: BTreeMap<String, String>,
    pub feeds: BTreeMap<String, String>,
}
pub struct Build {
    pub files: BTreeMap<String, Vec<u8>>,
    pub manifest: Manifest,
    pub identities: BTreeMap<String, Identity>,
}
fn page(
    s: &Snapshot,
    title: &str,
    summary: &str,
    route: &str,
    content: &str,
    image: &str,
    article: bool,
) -> Result<String> {
    let p = &s.config;
    let custom = format!("themes/{}/page.html", p.theme);
    let template = s
        .files
        .get(&custom)
        .map(|b| std::str::from_utf8(b))
        .transpose()?
        .unwrap_or(include_str!("../../../themes/default/page.html"));
    let mut env = minijinja::Environment::new();
    env.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    env.set_fuel(Some(100000));
    env.add_template("page.html", template)?;
    Ok(env.get_template("page.html")?.render(minijinja::context! {title=>title, summary=>summary, language=>&p.language, name=>&p.name, tagline=>&p.tagline, author=>&p.author, base=>p.base_url.trim_end_matches('/'), canonical=>absolute(p,route), content=>content, image=>image, kind=>if article {"article"} else {"website"}})?)
}
fn list(s: &Snapshot, articles: &[&Article]) -> String {
    if articles.is_empty() {
        return "<p class=\"empty\">No articles published yet.</p>".into();
    }
    articles.iter().map(|a| format!("<article class=\"entry\"><div class=\"edition\">{}</div><h2><a href=\"{}\">{}</a></h2><p>{}</p><div class=\"date\">{}</div></article>", escape(&s.config.series[&a.meta.series].title), escape(&absolute(&s.config,&article_route(&s.config,&a.meta))), escape(&a.meta.title), escape(&a.meta.summary), a.meta.published_at.map(|d|d.format("%-d %B %Y").to_string()).unwrap_or_default())).collect()
}
fn article_body(s: &Snapshot, a: &Article, offline: bool) -> String {
    let mut out = format!(
        "<article><div class=\"edition\">{}</div><h1>{}</h1><div class=\"date\">{}</div><p class=\"summary\">{}</p>",
        escape(
            s.config
                .series
                .get(&a.meta.series)
                .map(|s| s.title.as_str())
                .unwrap_or(&a.meta.series)
        ),
        escape(&a.meta.title),
        a.meta
            .published_at
            .map(|d| d.format("%-d %B %Y").to_string())
            .unwrap_or_else(|| "Draft · date unset".into()),
        escape(&a.meta.summary)
    );
    if let Some(img) = &a.meta.header_image
        && let Ok(local) = image_path(img)
        && (!offline || local.is_some())
    {
        out += &format!(
            "<figure><img src=\"{}\" alt=\"{}\"></figure>",
            escape(
                &local
                    .map(|p| absolute(&s.config, &p))
                    .unwrap_or(img.clone())
            ),
            escape(&a.meta.title)
        );
    }
    out += &markdown(&a.body, &s.config, offline);
    if let Some(d) = a.meta.updated_at {
        out += &format!(
            "<p class=\"date\">Updated {}</p>",
            d.format("%-d %B %Y, %H:%M %:z")
        );
    }
    out += "</article>";
    out
}
fn rss(s: &Snapshot, articles: &[&Article], route: &str, title: &str) -> String {
    let p = &s.config;
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\" xmlns:content=\"http://purl.org/rss/1.0/modules/content/\" xmlns:atom=\"http://www.w3.org/2005/Atom\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\"><channel><title>{}</title><link>{}</link><description>{}</description><language>{}</language><atom:link href=\"{}\" rel=\"self\" type=\"application/rss+xml\"/>",
        escape(title),
        escape(&p.base_url),
        escape(&p.description),
        escape(&p.language),
        escape(&absolute(p, route))
    );
    for a in articles.iter().take(p.feeds.limit) {
        let m = &a.meta;
        let date = m.published_at.unwrap();
        let body = if p.feeds.full_content {
            markdown(&a.body, p, false)
        } else {
            format!("<p>{}</p>", escape(&m.summary))
        };
        out += &format!(
            "<item><title>{}</title><link>{}</link><guid isPermaLink=\"false\">urn:uuid:{}</guid><pubDate>{}</pubDate><dcterms:modified>{}</dcterms:modified><dc:creator>{}</dc:creator><description>{}</description><content:encoded>{}</content:encoded>",
            escape(&m.title),
            escape(&absolute(p, &article_route(p, m))),
            m.id,
            date.format("%a, %d %b %Y %H:%M:%S %z"),
            m.updated_at.unwrap_or(date).to_rfc3339(),
            escape(&p.author),
            escape(&m.summary),
            escape(&body)
        );
        if let Some(img) = &m.header_image
            && let Ok(Some(path)) = image_path(img)
            && let Some(bytes) = s.files.get(&path)
            && let Ok((_, mime)) = detect_image(bytes)
        {
            out += &format!(
                "<enclosure url=\"{}\" length=\"{}\" type=\"{}\"/>",
                escape(&absolute(p, &path)),
                bytes.len(),
                mime
            );
        }
        out += "</item>";
    }
    out += "</channel></rss>";
    out
}
pub fn build(s: &Snapshot, ledger: &State, private: bool) -> Result<Build> {
    let (articles, parse_issues) = s.articles();
    ensure!(
        parse_issues.is_empty(),
        "Cannot build malformed source: {}",
        serde_json::to_string(&parse_issues)?
    );
    let selected: Vec<&Article> = articles
        .iter()
        .filter(|a| private || a.meta.status != Status::Draft)
        .collect();
    let diagnostics = diagnostics(s, ledger);
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| {
            d.severity == "error"
                && (!private)
                && (d.path == "publication.toml"
                    || selected.iter().any(|a| a.path == d.path)
                    || d.message.contains("Duplicate"))
        })
        .collect();
    ensure!(
        errors.is_empty(),
        "Preflight failed: {}",
        serde_json::to_string(&errors)?
    );
    let mut files = BTreeMap::new();
    let all_routes: Vec<String> = selected
        .iter()
        .map(|a| article_route(&s.config, &a.meta))
        .collect();
    let mut identities = BTreeMap::new();
    let mut article_hashes = BTreeMap::new();
    for a in &selected {
        let route = article_route(&s.config, &a.meta);
        let image = a
            .meta
            .header_image
            .as_ref()
            .map(|i| {
                if i.starts_with("/media/") {
                    absolute(&s.config, i)
                } else {
                    i.clone()
                }
            })
            .unwrap_or_default();
        files.insert(
            format!("{route}index.html"),
            page(
                s,
                &a.meta.title,
                &a.meta.summary,
                &route,
                &article_body(s, a, private),
                &image,
                true,
            )?
            .into_bytes(),
        );
        article_hashes.insert(a.meta.id.to_string(), a.hash.clone());
        if let Some(d) = a.meta.published_at {
            identities.insert(
                a.meta.id.to_string(),
                Identity {
                    url: absolute(&s.config, &route),
                    published_at: d.to_rfc3339(),
                },
            );
        }
        let mut media = Vec::new();
        if let Some(img) = &a.meta.header_image {
            media.push(img.clone());
        }
        for e in Parser::new_ext(&a.body, options()) {
            if let Event::Start(Tag::Image { dest_url, .. }) = e {
                media.push(dest_url.to_string());
            }
        }
        for img in media {
            if let Ok(Some(path)) = image_path(&img)
                && let Some(data) = s.files.get(&path)
            {
                ensure!(detect_image(data).is_ok(), "Invalid media");
                files.insert(path, data.clone());
            }
        }
    }
    files.insert(
        "index.html".into(),
        page(
            s,
            &s.config.name,
            &s.config.description,
            "",
            &list(s, &selected),
            "",
            false,
        )?
        .into_bytes(),
    );
    files.insert(
        "about/index.html".into(),
        page(
            s,
            "About",
            &s.config.description,
            "about/",
            &format!(
                "<h1>About {}</h1>{}",
                escape(&s.config.name),
                markdown(&s.config.about, &s.config, private)
            ),
            "",
            false,
        )?
        .into_bytes(),
    );
    for (id, series) in &s.config.series {
        let subset: Vec<_> = selected
            .iter()
            .copied()
            .filter(|a| &a.meta.series == id)
            .collect();
        files.insert(
            format!("{}/index.html", series.route),
            page(
                s,
                &series.title,
                &s.config.description,
                &format!("{}/", series.route),
                &format!(
                    "<h1>{}</h1><p><a href=\"{}\">Subscribe to this series</a></p>{}",
                    escape(&series.title),
                    escape(&absolute(&s.config, &format!("{}/rss.xml", series.route))),
                    list(s, &subset)
                ),
                "",
                false,
            )?
            .into_bytes(),
        );
        if !private {
            files.insert(
                format!("{}/rss.xml", series.route),
                rss(
                    s,
                    &subset,
                    &format!("{}/rss.xml", series.route),
                    &series.title,
                )
                .into_bytes(),
            );
        }
    }
    let mut archives: BTreeMap<String, Vec<&Article>> = BTreeMap::new();
    archives.insert("archive/".into(), selected.clone());
    for a in &selected {
        if let Some(d) = a.meta.published_at {
            for route in [
                d.format("archive/%Y/").to_string(),
                d.format("archive/%Y/%m/").to_string(),
            ] {
                archives.entry(route).or_default().push(a);
            }
        }
    }
    for (route, subset) in archives {
        files.insert(
            format!("{route}index.html"),
            page(
                s,
                "Archive",
                &s.config.description,
                &route,
                &format!("<h1>Archive</h1>{}", list(s, &subset)),
                "",
                false,
            )?
            .into_bytes(),
        );
    }
    let css_key = format!("themes/{}/style.css", s.config.theme);
    files.insert(
        "assets/style.css".into(),
        s.files
            .get(&css_key)
            .cloned()
            .unwrap_or_else(|| include_bytes!("../../../themes/default/style.css").to_vec()),
    );
    files.insert("favicon.svg".into(),b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\"><rect width=\"64\" height=\"64\" rx=\"10\" fill=\"#42653c\"/><path d=\"M20 14h24v6H26v10h16v6H26v14h-6z\" fill=\"#fff\"/></svg>".to_vec());
    for (name, data) in &s.files {
        if matches!(name.as_str(), "static/favicon.png" | "static/favicon.ico") {
            files.insert(name.trim_start_matches("static/").into(), data.clone());
        }
    }
    files.insert(".nojekyll".into(), Vec::new());
    if !private {
        files.insert(
            "rss.xml".into(),
            rss(s, &selected, "rss.xml", &s.config.name).into_bytes(),
        );
        let p = &s.config;
        let updated = selected
            .iter()
            .filter_map(|a| a.meta.updated_at.or(a.meta.published_at))
            .max()
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(|| "1970-01-01T00:00:00+00:00".into());
        let mut atom = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><feed xmlns=\"http://www.w3.org/2005/Atom\"><id>{}</id><title>{}</title><updated>{}</updated><author><name>{}</name></author><link href=\"{}\"/><link rel=\"self\" href=\"{}\"/>",
            escape(&absolute(p, "atom.xml")),
            escape(&p.name),
            updated,
            escape(&p.author),
            escape(&p.base_url),
            escape(&absolute(p, "atom.xml"))
        );
        let mut items = Vec::new();
        for a in selected.iter().take(p.feeds.limit) {
            let m = &a.meta;
            let published = m.published_at.unwrap();
            let content = if p.feeds.full_content {
                markdown(&a.body, p, false)
            } else {
                format!("<p>{}</p>", escape(&m.summary))
            };
            atom += &format!(
                "<entry><id>urn:uuid:{}</id><title>{}</title><link href=\"{}\"/><published>{}</published><updated>{}</updated><summary>{}</summary><content type=\"html\">{}</content></entry>",
                m.id,
                escape(&m.title),
                escape(&absolute(p, &article_route(p, m))),
                published.to_rfc3339(),
                m.updated_at.unwrap_or(published).to_rfc3339(),
                escape(&m.summary),
                escape(&content)
            );
            let mut item = serde_json::json!({"id":format!("urn:uuid:{}",m.id),"url":absolute(p,&article_route(p,m)),"title":m.title,"summary":m.summary,"content_html":content,"date_published":published.to_rfc3339(),"date_modified":m.updated_at.unwrap_or(published).to_rfc3339(),"tags":m.tags,"authors":[{"name":p.author}]});
            if let Some(img) = &m.header_image {
                item["image"] = serde_json::json!(if img.starts_with("/media/") {
                    absolute(p, img)
                } else {
                    img.clone()
                });
            }
            items.push(item);
        }
        atom += "</feed>";
        files.insert("atom.xml".into(), atom.into_bytes());
        files.insert("feed.json".into(),serde_json::to_vec_pretty(&serde_json::json!({"version":"https://jsonfeed.org/version/1.1","title":p.name,"home_page_url":p.base_url,"feed_url":absolute(p,"feed.json"),"description":p.description,"language":p.language,"items":items}))?);
        let mut routes = vec!["".to_string(), "about/".into(), "archive/".into()];
        routes.extend(all_routes);
        routes.extend(p.series.values().map(|v| format!("{}/", v.route)));
        let urls: String = routes
            .iter()
            .map(|r| format!("<url><loc>{}</loc></url>", escape(&absolute(p, r))))
            .collect();
        files.insert("sitemap.xml".into(),format!("<?xml version=\"1.0\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">{urls}</urlset>").into_bytes());
        files.insert(
            "robots.txt".into(),
            format!(
                "User-agent: *\nAllow: /\nSitemap: {}\n",
                absolute(p, "sitemap.xml")
            )
            .into_bytes(),
        );
        feed_check_files(&files)?;
    } else {
        files.insert(
            "robots.txt".into(),
            b"User-agent: *\nDisallow: /\n".to_vec(),
        );
    }
    let file_hashes: BTreeMap<_, _> = files.iter().map(|(p, b)| (p.clone(), hash(b))).collect();
    let feeds = file_hashes
        .iter()
        .filter(|(p, _)| p.ends_with("rss.xml") || *p == "atom.xml" || *p == "feed.json")
        .map(|(p, h)| (p.clone(), h.clone()))
        .collect();
    let manifest = Manifest {
        schema: SCHEMA,
        source_hash: s.hash.clone(),
        output_hash: tree_hash(&files),
        private_preview: private,
        files: file_hashes,
        articles: article_hashes,
        feeds,
    };
    files.insert(
        "build-manifest.json".into(),
        serde_json::to_vec_pretty(&manifest)?,
    );
    Ok(Build {
        files,
        manifest,
        identities,
    })
}
pub fn write_build(root: &Path, output: &Path, build: &Build) -> Result<()> {
    let root = root.canonicalize()?;
    let output = if output.is_absolute() {
        output.to_path_buf()
    } else {
        root.join(output)
    };
    let relative = output
        .strip_prefix(&root)
        .context("Generated output must stay within the publication folder")?
        .to_str()
        .context("Output path must be UTF-8")?;
    ensure!(
        relative == "output" || relative == ".omapress/preview",
        "Output must be output/ or .omapress/preview/"
    );
    let output = safe_path(&root, relative)?;
    let parent = output.parent().unwrap();
    fs::create_dir_all(parent)?;
    let stage = tempfile::tempdir_in(parent)?;
    for (name, bytes) in &build.files {
        let path = safe_path(stage.path(), name)?;
        atomic_write(&path, bytes)?;
    }
    let backup = parent.join(format!(".omapress-old-{}", uuid::Uuid::new_v4()));
    if output.exists() {
        fs::rename(&output, &backup)?;
    }
    if let Err(e) = fs::rename(stage.path(), &output) {
        if backup.exists() {
            let _ = fs::rename(&backup, &output);
        }
        return Err(e.into());
    }
    if backup.exists() {
        fs::remove_dir_all(backup)?;
    }
    Ok(())
}
pub fn feed_check_files(files: &BTreeMap<String, Vec<u8>>) -> Result<()> {
    for (name, bytes) in files {
        if name.ends_with("rss.xml") {
            let doc = roxmltree::Document::parse(std::str::from_utf8(bytes)?)
                .context("Invalid RSS XML")?;
            ensure!(
                doc.root_element().tag_name().name() == "rss"
                    && doc.root_element().attribute("version") == Some("2.0"),
                "RSS 2.0 root required"
            );
            let channel = doc
                .root_element()
                .children()
                .find(|n| n.has_tag_name("channel"))
                .context("Missing RSS channel")?;
            for field in ["title", "link", "description"] {
                ensure!(
                    channel
                        .children()
                        .any(|n| n.has_tag_name(field) && n.text().is_some()),
                    "Missing RSS channel {field}"
                );
            }
            let mut ids = BTreeSet::new();
            for item in channel.children().filter(|n| n.has_tag_name("item")) {
                let value = |tag: &str| {
                    item.children()
                        .find(|n| n.has_tag_name(tag))
                        .and_then(|n| n.text())
                };
                ensure!(
                    value("title").is_some_and(|v| !v.is_empty()),
                    "Missing RSS title"
                );
                checked_link(value("link").context("Missing RSS link")?)?;
                ensure!(
                    ids.insert(value("guid").context("Missing GUID")?.to_string()),
                    "Duplicate RSS GUID"
                );
                DateTime::parse_from_rfc2822(value("pubDate").context("Missing pubDate")?)?;
                ensure!(
                    item.children().any(|n| n.tag_name().namespace()
                        == Some("http://purl.org/rss/1.0/modules/content/")
                        && n.tag_name().name() == "encoded"),
                    "Missing full-content field"
                );
            }
        } else if name == "atom.xml" {
            let doc = roxmltree::Document::parse(std::str::from_utf8(bytes)?)?;
            ensure!(
                doc.root_element()
                    .has_tag_name(("http://www.w3.org/2005/Atom", "feed")),
                "Invalid Atom root"
            );
            let mut ids = BTreeSet::new();
            for entry in doc
                .descendants()
                .filter(|n| n.has_tag_name(("http://www.w3.org/2005/Atom", "entry")))
            {
                for tag in ["id", "title", "published", "updated"] {
                    let v = entry
                        .children()
                        .find(|n| n.tag_name().name() == tag)
                        .and_then(|n| n.text())
                        .context("Missing Atom entry field")?;
                    if tag == "id" {
                        ensure!(ids.insert(v), "Duplicate Atom ID");
                    }
                    if tag == "published" || tag == "updated" {
                        DateTime::parse_from_rfc3339(v)?;
                    }
                }
            }
        } else if name == "feed.json" {
            let v: serde_json::Value = serde_json::from_slice(bytes)?;
            ensure!(
                v["version"] == "https://jsonfeed.org/version/1.1",
                "Invalid JSON Feed version"
            );
            let mut ids = BTreeSet::new();
            for item in v["items"].as_array().context("Missing JSON Feed items")? {
                ensure!(
                    ids.insert(item["id"].as_str().context("Missing JSON Feed ID")?),
                    "Duplicate JSON Feed ID"
                );
                checked_link(item["url"].as_str().context("Missing JSON Feed URL")?)?;
                DateTime::parse_from_rfc3339(
                    item["date_published"]
                        .as_str()
                        .context("Missing publication date")?,
                )?;
                ensure!(item["content_html"].is_string(), "Missing content_html");
            }
        }
    }
    Ok(())
}
#[derive(Serialize)]
pub struct XExport {
    pub html: String,
    pub preview_html: String,
    pub text: String,
    pub caption: String,
    pub warnings: Vec<String>,
}
pub fn export_x(s: &Snapshot, a: &Article) -> XExport {
    let mut warnings = BTreeSet::new();
    let mut plain = String::new();
    let mut link_stack = Vec::new();
    for e in Parser::new_ext(&a.body, options()) {
        match e {
            Event::Text(t) | Event::Code(t) | Event::Html(t) | Event::InlineHtml(t) => {
                plain.push_str(&t)
            }
            Event::Start(Tag::Item) => plain.push_str("• "),
            Event::Start(Tag::Link { dest_url, .. }) => link_stack.push(dest_url.to_string()),
            Event::End(TagEnd::Link) => {
                if let Some(url) = link_stack.pop() {
                    plain += &format!(" ({url})");
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                warnings.insert("Images must be uploaded manually in X; image references are retained in the export.".to_string());
                plain += &format!("[Image: {dest_url}] ");
            }
            Event::Start(Tag::Table(_)) => {
                warnings.insert(
                    "X may not preserve tables; check the exported table before publishing.".into(),
                );
            }
            Event::Start(Tag::FootnoteDefinition(_)) => {
                warnings.insert("Footnotes may require manual adjustment in X.".into());
            }
            Event::End(
                TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::List(_),
            ) => plain.push_str("\n\n"),
            Event::End(TagEnd::Item | TagEnd::TableRow) => plain.push('\n'),
            Event::End(TagEnd::TableCell) => plain.push('\t'),
            Event::SoftBreak | Event::HardBreak => plain.push('\n'),
            Event::Rule => plain.push_str("\n────────\n"),
            _ => {}
        }
    }
    let p = &s.config;
    let mut header_html = String::new();
    let mut header_text = String::new();
    for key in &p.x_export_order {
        let value = match key.as_str() {
            "masthead" => p.name.clone(),
            "edition" => p
                .series
                .get(&a.meta.series)
                .map(|s| s.title.clone())
                .unwrap_or(a.meta.series.clone()),
            "title" => a.meta.title.clone(),
            "date" => a
                .meta
                .published_at
                .map(|d| d.format("%-d %B %Y").to_string())
                .unwrap_or_default(),
            _ => String::new(),
        };
        let tag = if key == "title" {
            "h1"
        } else if key == "masthead" {
            "h2"
        } else {
            "p"
        };
        header_html += &format!("<{tag}>{}</{tag}>\n", escape(&value));
        header_text += &format!("{value}\n\n");
    }
    if a.meta.header_image.is_some() {
        warnings.insert("Upload the header image separately in X.".into());
    }
    if a.body.contains('<') {
        warnings.insert("Raw HTML is exported as visible text.".into());
    }
    XExport {
        preview_html: (header_html.clone() + &markdown(&a.body, p, true)).replace(
            &format!("{}/media/", p.base_url.trim_end_matches('/')),
            &format!("file://{}/media/", s.root.display()),
        ),
        html: header_html + &markdown(&a.body, p, false),
        text: header_text + plain.trim(),
        caption: a.meta.x_caption.clone(),
        warnings: warnings.into_iter().collect(),
    }
}
