//! Private destination receipts. Remote uncertainty must never trigger blind retries.
use crate::{model::*, render, storage::*};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub article_hash: String,
    pub status: String,
    pub url: String,
    pub remote_id: String,
    pub updated_at: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub review_hash: String,
    #[serde(default)]
    pub schedule_hash: String,
    #[serde(default)]
    pub scheduled_at: String,
    #[serde(default)]
    pub post_audience: String,
    #[serde(default)]
    pub email_audience: String,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    schema: u32,
    entries: BTreeMap<String, BTreeMap<String, Receipt>>,
}
fn load(root: &Path) -> Result<Ledger> {
    let path = safe_path(root, ".omapress/distribution.json")?;
    if !path.exists() {
        return Ok(Ledger {
            schema: 1,
            ..Default::default()
        });
    }
    let l: Ledger = serde_json::from_slice(&read_bounded(&path, 8 * 1024 * 1024)?)
        .context("Distribution history is corrupt; restore it before continuing")?;
    ensure!(l.schema == 1, "Unsupported distribution history version");
    Ok(l)
}
fn save(root: &Path, l: &Ledger) -> Result<()> {
    atomic_write(
        &safe_path(root, ".omapress/distribution.json")?,
        &serde_json::to_vec_pretty(l)?,
    )
}
fn article(root: &Path, path: &str, expected: &str) -> Result<(Snapshot, Article)> {
    let s = snapshot(root)?;
    s.expected(expected)?;
    let a = parse_article(path, s.files.get(path).context("Article not found")?)?;
    Ok((s, a))
}
pub fn plan(root: &Path, path: &str, expected: &str) -> Result<Value> {
    let (s, a) = article(root, path, expected)?;
    let ledger = load(root)?;
    let receipts = ledger
        .entries
        .get(&a.meta.id.to_string())
        .cloned()
        .unwrap_or_default();
    let site = state(root)?;
    let site_current = site
        .deployments
        .last()
        .is_some_and(|d| d.articles.get(&a.meta.id.to_string()) == Some(&a.hash));
    let route = s
        .config
        .series
        .get(&a.meta.series)
        .context("Unknown series")?;
    let targets: Vec<_> = ["website", "x", "substack"]
        .iter()
        .map(|target| {
            let receipt = receipts.get(*target);
            let status = if *target == "website" {
                if site_current {
                    "published"
                } else {
                    "not_published"
                }
            } else {
                receipt
                    .map(|r| {
                        if r.article_hash != a.hash {
                            "changed"
                        } else {
                            r.status.as_str()
                        }
                    })
                    .unwrap_or("not_published")
            };
            json!({"target":target,"status":status,"receipt":receipt})
        })
        .collect();
    let errors: Vec<_> = render::diagnostics(&s, &site)
        .into_iter()
        .filter(|d| d.severity == "error")
        .collect();
    let x = crate::x_article::payload(&a).and_then(|p| {
        crate::x_article::media_paths(&s, &p)?;
        Ok(p)
    });
    Ok(
        json!({"article":path,"article_id":a.meta.id,"article_hash":a.hash,"source_hash":s.hash,
        "title":a.meta.title,"canonical_url":format!("{}/{}/{}/",s.config.base_url.trim_end_matches('/'),route.route.trim_matches('/'),a.meta.slug),
        "targets":targets,"ready":a.meta.status!=Status::Draft&&errors.is_empty(),"diagnostics":errors,
        "x":render::export_x(&s,&a),"x_api_supported":x.is_ok(),"x_api_issue":x.err().map(|e|e.to_string()),
        "substack":{"title":a.meta.title,"subtitle":a.meta.summary,"html":render::markdown(&a.body,&s.config,false),
            "preview_html":render::markdown(&a.body,&s.config,true),"text":a.body,"header_image":a.meta.header_image},
        "website_scope":"Website publishing includes every Ready/Published article in this publication."}),
    )
}
pub fn confirm(root: &Path, path: &str, expected: &str, target: &str, url: &str) -> Result<Value> {
    ensure!(["x", "substack"].contains(&target), "Select X or Substack");
    let u = url::Url::parse(url)?;
    ensure!(
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && u.port().is_none(),
        "Use a public HTTPS article URL"
    );
    if target == "x" {
        ensure!(
            ["x.com", "www.x.com"].contains(&u.host_str().unwrap_or(""))
                && (u.path().contains("/status/") || u.path().contains("/article/")),
            "Use an X Article URL"
        );
    } else {
        ensure!(
            u.path().starts_with("/p/") && u.path().len() > 3,
            "Use the published Substack /p/article URL (custom domains supported)"
        );
    }
    let _lock = lock(root)?;
    let (_, a) = article(root, path, expected)?;
    let mut l = load(root)?;
    l.entries.entry(a.meta.id.to_string()).or_default().insert(
        target.into(),
        Receipt {
            article_hash: a.hash,
            status: "confirmed_by_user".into(),
            url: url.into(),
            remote_id: String::new(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            account_id: String::new(),
            review_hash: String::new(),
            schedule_hash: String::new(),
            scheduled_at: String::new(),
            post_audience: String::new(),
            email_audience: String::new(),
        },
    );
    save(root, &l)?;
    plan(root, path, expected)
}
pub fn x_action(root: &Path, path: &str, expected: &str, publish: bool) -> Result<Value> {
    let _lock = lock(root)?;
    let (s, a) = article(root, path, expected)?;
    ensure!(
        a.meta.status != Status::Draft,
        "Mark the reviewed article Ready first"
    );
    ensure!(
        !render::diagnostics(&s, &state(root)?)
            .iter()
            .any(|d| d.severity == "error"),
        "Resolve publication checks first"
    );
    let mut payload = crate::x_article::payload(&a)?;
    crate::x_article::media_paths(&s, &payload)?;
    let mut l = load(root)?;
    let id = a.meta.id.to_string();
    let previous = l.entries.get(&id).and_then(|m| m.get("x")).cloned();
    if let Some(r) = &previous {
        ensure!(
            r.article_hash == a.hash,
            "Article changed after X preparation. Edit the existing X article manually; creating a duplicate is blocked."
        );
        if ["published", "confirmed_by_user"].contains(&r.status.as_str()) {
            return plan(root, path, expected);
        }
        ensure!(
            r.status == "draft",
            "Previous X request has an unknown outcome. Check X and record its published URL before retrying."
        );
        if !publish {
            return plan(root, path, expected);
        }
    }
    let (token, account) = crate::x_article::credentials()?;
    if let Some(r) = &previous {
        ensure!(
            r.account_id == account,
            "The X draft belongs to a different or legacy connection. Verify it in X before proceeding."
        );
    }
    let mut r = previous.unwrap_or(Receipt {
        article_hash: a.hash.clone(),
        status: String::new(),
        url: String::new(),
        remote_id: String::new(),
        updated_at: String::new(),
        account_id: account.clone(),
        review_hash: String::new(),
        schedule_hash: String::new(),
        scheduled_at: String::new(),
        post_audience: String::new(),
        email_audience: String::new(),
    });
    if r.remote_id.is_empty() {
        crate::x_article::upload_media(&s, &token, &mut payload)?;
        r.status = "unknown".into();
        r.updated_at = chrono::Utc::now().to_rfc3339();
        l.entries
            .entry(id.clone())
            .or_default()
            .insert("x".into(), r.clone());
        save(root, &l)?;
        let draft = crate::x_article::request(&token, "/2/articles/draft", &payload)?;
        r.remote_id = draft["data"]["id"]
            .as_str()
            .context("X returned no article ID; check X before retrying")?
            .into();
        ensure!(
            !r.remote_id.is_empty() && r.remote_id.chars().all(|c| c.is_ascii_digit()),
            "Invalid X article ID"
        );
        r.status = "draft".into();
        r.url = "https://x.com/compose/articles".into();
        l.entries
            .get_mut(&id)
            .unwrap()
            .insert("x".into(), r.clone());
        save(root, &l)?;
    }
    if publish {
        r.status = "unknown".into();
        l.entries
            .get_mut(&id)
            .unwrap()
            .insert("x".into(), r.clone());
        save(root, &l)?;
        let v = crate::x_article::request(
            &token,
            &format!("/2/articles/{}/publish", r.remote_id),
            &json!({}),
        )?;
        let post = v["data"]["post_id"]
            .as_str()
            .context("X returned no post ID; check X before retrying")?;
        ensure!(
            !post.is_empty() && post.chars().all(|c| c.is_ascii_digit()),
            "Invalid X post ID"
        );
        r.url = format!("https://x.com/i/status/{post}");
        r.status = "published".into();
        r.updated_at = chrono::Utc::now().to_rfc3339();
        l.entries.get_mut(&id).unwrap().insert("x".into(), r);
        save(root, &l)?;
    }
    plan(root, path, expected)
}

/// Prepare the website remote head and selected destinations for one explicit review.
pub fn review(
    root: &Path,
    path: &str,
    expected: &str,
    website: bool,
    x: bool,
    substack: bool,
) -> Result<Value> {
    ensure!(website || x || substack, "Select at least one destination");
    let p = plan(root, path, expected)?;
    ensure!(
        p["ready"] == true,
        "Mark the article Ready and resolve publication checks"
    );
    if x {
        ensure!(
            p["x_api_supported"] == true,
            "X API does not support this article's formatting yet; use rich-copy export"
        );
    }
    let site = if website {
        crate::deploy::plan(root, expected)?
    } else {
        Value::Null
    };
    Ok(
        json!({"article":path,"source_hash":expected,"title":p["title"],"website":website,"x":x,"substack":substack,"site":site}),
    )
}
/// Partial success remains visible. Each adapter protects its own durable state.
pub fn publish_selected(
    root: &Path,
    path: &str,
    expected: &str,
    website: bool,
    x: bool,
    remote_head: &str,
    substack: bool,
) -> Result<Value> {
    ensure!(website || x || substack, "Select at least one destination");
    let p = plan(root, path, expected)?;
    ensure!(p["ready"] == true, "Reviewed article is not ready");
    if x {
        ensure!(
            p["x_api_supported"] == true,
            "X API does not support this article's formatting yet"
        );
        crate::x_article::token()?;
    }
    let mut results = Vec::new();
    if substack {
        let result = crate::substack::prepare(root, path, expected);
        results.push(match result {
            Ok(v) => json!({"target":"substack","result":v}),
            Err(e) => json!({"target":"substack","error":format!("{e:#}")}),
        });
    }
    if website {
        let result = crate::deploy::publish(root, expected, remote_head);
        results.push(match result {
            Ok(v) => json!({"target":"website","result":v}),
            Err(e) => json!({"target":"website","error":format!("{e:#}")}),
        });
    }
    if x {
        let result = x_action(root, path, expected, true);
        results.push(match result {
            Ok(v) => json!({"target":"x","result":v}),
            Err(e) => json!({"target":"x","error":format!("{e:#}")}),
        });
    }
    Ok(json!({"results":results,"distribution":plan(root,path,expected)?}))
}

/// Caller holds the publication lock; local preparation never implies publication.
pub(crate) fn substack_prepared(root: &Path, a: &Article, id: &str) -> Result<()> {
    let mut ledger = load(root)?;
    let entries = ledger.entries.entry(a.meta.id.to_string()).or_default();
    if let Some(previous) = entries.get("substack") {
        ensure!(
            previous.status == "awaiting_browser",
            "A Substack URL is already recorded. Edit the existing post to avoid a duplicate."
        );
    }
    entries.insert(
        "substack".into(),
        Receipt {
            article_hash: a.hash.clone(),
            status: "awaiting_browser".into(),
            url: String::new(),
            remote_id: id.into(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            account_id: String::new(),
            review_hash: String::new(),
            schedule_hash: String::new(),
            scheduled_at: String::new(),
            post_audience: String::new(),
            email_audience: String::new(),
        },
    );
    save(root, &ledger)
}

// The gateway caller retains the publication lock across remote work and receipt writes.
pub(crate) fn gateway_receipt(root: &Path, id: &str) -> Result<Option<Receipt>> {
    Ok(load(root)?
        .entries
        .get(id)
        .and_then(|e| e.get("substack"))
        .cloned())
}
pub(crate) fn save_gateway_receipt(root: &Path, id: &str, receipt: Receipt) -> Result<()> {
    let mut ledger = load(root)?;
    ledger
        .entries
        .entry(id.into())
        .or_default()
        .insert("substack".into(), receipt);
    save(root, &ledger)
}
