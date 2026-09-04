use omapress_core::{
    model::*,
    preview,
    protocol::{self, Request},
    render,
    storage::*,
};
use serde_json::json;
use std::fs;
fn fixture() -> (tempfile::TempDir, Snapshot) {
    let t = tempfile::tempdir().unwrap();
    let s = init(
        &t.path().join("pub"),
        "Fixture publication",
        "https://example.com/news",
        "Test Author",
    )
    .unwrap();
    (t, s)
}
fn article(id: &str, slug: &str, status: Status, body: &str) -> String {
    article_text(
        &ArticleMeta {
            schema: 1,
            id: uuid::Uuid::parse_str(id).unwrap(),
            title: "An editorial title & more".into(),
            series: "today-in-omarchy".into(),
            status,
            published_at: Some("2026-01-02T21:00:00+00:00".parse().unwrap()),
            updated_at: None,
            summary: "A useful <summary>".into(),
            slug: slug.into(),
            header_image: None,
            x_caption: "A caption with a link https://example.com".into(),
            x_url: None,
            tags: vec!["rust".into()],
            source_window: None,
        },
        body,
    )
    .unwrap()
}
const ID: &str = "018fc5c0-2f54-7ea8-aeb2-75e566f38345";
fn ready(s: &Snapshot) -> Snapshot {
    save_article(&s.root,"content/today-in-omarchy/story.md",&article(ID,"story",Status::Ready,"## Heading\n\nA **bold** point and [source](https://example.com/?a=1&b=2).\n\n- One\n- Two\n\n```rust\nlet n = 42;\n```\n"),&s.hash).unwrap()
}
#[test]
fn deterministic_full_site_and_three_feeds() {
    let (_t, s) = fixture();
    let s = ready(&s);
    let a = render::build(&s, &state(&s.root).unwrap(), false).unwrap();
    let b = render::build(&s, &state(&s.root).unwrap(), false).unwrap();
    assert_eq!(a.files, b.files);
    assert_eq!(a.manifest.output_hash, b.manifest.output_hash);
    for path in [
        "index.html",
        "today-in-omarchy/story/index.html",
        "today-in-omarchy/rss.xml",
        "the-unofficial-week-in-omarchy/rss.xml",
        "rss.xml",
        "atom.xml",
        "feed.json",
        "archive/2026/01/index.html",
        "archive/2026/index.html",
        "about/index.html",
    ] {
        assert!(a.files.contains_key(path), "{path}");
    }
    render::feed_check_files(&a.files).unwrap();
}
#[test]
fn drafts_and_unreferenced_media_never_publish() {
    let (_t, s) = fixture();
    let s = ready(&s);
    let s = save_article(
        &s.root,
        "content/draft.md",
        &article(
            "118fc5c0-2f54-7ea8-aeb2-75e566f38345",
            "secret-draft",
            Status::Draft,
            "TOP SECRET editorial draft",
        ),
        &s.hash,
    )
    .unwrap();
    fs::write(s.root.join("media/secret.png"), b"\x89PNG\r\n\x1a\nprivate").unwrap();
    fs::write(s.root.join("static/.env"), b"SECRET_TOKEN=private").unwrap();
    let s = snapshot(&s.root).unwrap();
    let b = render::build(&s, &state(&s.root).unwrap(), false).unwrap();
    assert!(
        !b.files
            .keys()
            .any(|p| p.contains("secret") || p.contains(".env"))
    );
    for (p, bytes) in &b.files {
        let text = String::from_utf8_lossy(bytes);
        assert!(!text.contains("TOP SECRET"), "{p}");
        assert!(!text.contains("SECRET_TOKEN"));
    }
}
#[test]
fn stale_source_save_cannot_overwrite() {
    let (_t, s) = fixture();
    let current = ready(&s);
    let err = save_article(
        &s.root,
        "content/today-in-omarchy/story.md",
        &article(ID, "story", Status::Ready, "overwrite"),
        &s.hash,
    )
    .unwrap_err();
    assert!(err.to_string().contains("Source changed"));
    assert_eq!(snapshot(&s.root).unwrap().hash, current.hash);
}
#[test]
fn published_identity_survives_correction_and_locks_url() {
    let (_t, s) = fixture();
    let s = ready(&s);
    let a = s.articles().0.remove(0);
    let mut ledger = state(&s.root).unwrap();
    ledger.identities.insert(
        ID.into(),
        Identity {
            url: absolute(&s.config, &article_route(&s.config, &a.meta)),
            published_at: a.meta.published_at.unwrap().to_rfc3339(),
        },
    );
    save_state(&s.root, &ledger).unwrap();
    let updated = save_article(
        &s.root,
        &a.path,
        &article(ID, "story", Status::Ready, "Corrected text"),
        &s.hash,
    )
    .unwrap();
    let changed = updated.articles().0.remove(0);
    assert!(changed.meta.updated_at.is_some());
    let b = render::build(&updated, &ledger, false).unwrap();
    assert!(String::from_utf8_lossy(&b.files["rss.xml"]).contains(&format!("urn:uuid:{ID}")));
    assert!(
        save_article(
            &s.root,
            &a.path,
            &article(ID, "different", Status::Ready, "text"),
            &updated.hash
        )
        .is_err()
    );
}
#[test]
fn changing_published_id_is_rejected() {
    let (_t, s) = fixture();
    let s = ready(&s);
    assert!(
        save_article(
            &s.root,
            "content/today-in-omarchy/story.md",
            &article(
                "118fc5c0-2f54-7ea8-aeb2-75e566f38345",
                "story",
                Status::Ready,
                "Text"
            ),
            &s.hash
        )
        .is_err()
    );
}
#[test]
fn malicious_links_block_publication_and_never_render_active() {
    let (_t, s) = fixture();
    let text = article(
        ID,
        "unsafe",
        Status::Ready,
        "[click](javascript:alert%281%29)\n\n<script>alert('bad')</script>\n",
    );
    let s = save_article(&s.root, "content/unsafe.md", &text, &s.hash).unwrap();
    assert!(render::build(&s, &state(&s.root).unwrap(), false).is_err());
    let html = render::markdown(&s.articles().0[0].body, &s.config, false);
    assert!(!html.contains("<script>"));
    assert!(!html.contains("href=\"javascript:"));
    assert!(html.contains("&lt;script&gt;"));
}
#[test]
fn symlink_input_and_output_are_rejected() {
    use std::os::unix::fs::symlink;
    let (t, s) = fixture();
    let outside = t.path().join("outside");
    fs::write(&outside, "secret").unwrap();
    symlink(&outside, s.root.join("content/link.md")).unwrap();
    assert!(snapshot(&s.root).is_err());
    fs::remove_file(s.root.join("content/link.md")).unwrap();
    let b = render::build(&s, &state(&s.root).unwrap(), false).unwrap();
    symlink(t.path(), s.root.join("output")).unwrap();
    assert!(render::write_build(&s.root, std::path::Path::new("output"), &b).is_err());
    assert_eq!(fs::read_to_string(outside).unwrap(), "secret");
}
#[test]
fn path_traversal_rejected() {
    for path in ["../x", "/etc/passwd", "x/../../a", "x\\y", "a\0b"] {
        assert!(safe_relative(path).is_err(), "{path}");
    }
    for image in [
        "/media/../.env",
        "/media/%2e%2e/a.png",
        "file:///etc/passwd",
        "/media/a.svg",
    ] {
        assert!(image_path(image).is_err(), "{image}");
    }
}
#[test]
fn feed_validation_detects_corruption_and_duplicate_ids() {
    let (_t, s) = fixture();
    let s = ready(&s);
    let mut b = render::build(&s, &state(&s.root).unwrap(), false).unwrap();
    let rss = String::from_utf8(b.files["rss.xml"].clone()).unwrap();
    let item = rss
        .split_once("<item>")
        .unwrap()
        .1
        .split_once("</item>")
        .unwrap()
        .0;
    let duplicate = rss.replace("</channel>", &format!("<item>{item}</item></channel>"));
    b.files.insert("rss.xml".into(), duplicate.into_bytes());
    assert!(render::feed_check_files(&b.files).is_err());
}
#[test]
fn output_rebuild_removes_deleted_artifacts() {
    let (_t, s) = fixture();
    let s = ready(&s);
    let b = render::build(&s, &state(&s.root).unwrap(), false).unwrap();
    render::write_build(&s.root, std::path::Path::new("output"), &b).unwrap();
    fs::write(s.root.join("output/stale.html"), "old").unwrap();
    render::write_build(&s.root, std::path::Path::new("output"), &b).unwrap();
    assert!(!s.root.join("output/stale.html").exists());
    assert_eq!(snapshot(&s.root).unwrap().hash, s.hash);
}
#[test]
fn x_export_keeps_body_links_caption_and_reports_tables() {
    let (_t, s) = fixture();
    let s = ready(&s);
    let mut a = s.articles().0.remove(0);
    a.body += "\n| A | B |\n|---|---|\n| 1 | 2 |\n";
    let x = render::export_x(&s, &a);
    assert!(x.text.contains("https://example.com/?a=1&b=2"));
    assert!(x.html.contains("<strong>bold</strong>"));
    assert!(x.warnings.iter().any(|v| v.contains("tables")));
    assert_eq!(x.caption, a.meta.x_caption);
    assert!(!x.html.contains("<nav"));
}
#[test]
fn failed_state_read_never_resets_history() {
    let (_t, s) = fixture();
    let _lock = lock(&s.root).unwrap();
    fs::write(s.root.join(".omapress/state.json"), "broken").unwrap();
    assert!(state(&s.root).is_err());
}
#[test]
fn schema_version_and_unknown_metadata_fail_closed() {
    let (_t, s) = fixture();
    let text = article(ID, "story", Status::Draft, "text");
    assert!(parse_article("x", text.replace("schema: 1", "schema: 99").as_bytes()).is_err());
    assert!(
        parse_article(
            "x",
            text.replace("schema: 1", "schema: 1\nunrecognised: true")
                .as_bytes()
        )
        .is_err()
    );
    assert!(
        protocol::execute(Request {
            schema: 99,
            command: "inspect".into(),
            path: s.root,
            args: json!({})
        })
        .is_err()
    );
}
#[test]
fn ready_validation_rejects_missing_title_and_bad_source() {
    let (_t, s) = fixture();
    let text = article(ID, "story", Status::Ready, "[source](not-a-url)")
        .replace("title: An editorial title & more", "title: ''");
    let s = save_article(&s.root, "content/story.md", &text, &s.hash).unwrap();
    let d = render::diagnostics(&s, &state(&s.root).unwrap());
    assert!(d.iter().any(|d| d.message.contains("Editorial title")));
    assert!(d.iter().any(|d| d.message.contains("Invalid source link")));
}
#[test]
fn concurrent_omapress_writes_are_locked() {
    let (_t, s) = fixture();
    let _lock = lock(&s.root).unwrap();
    assert!(lock(&s.root).is_err());
    assert!(
        save_article(
            &s.root,
            "content/a.md",
            &article(ID, "story", Status::Draft, "body"),
            &s.hash
        )
        .is_err()
    );
}
#[test]
fn preview_host_and_path_are_restricted() {
    assert_eq!(
        preview::request_path(
            "GET /key/ HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n",
            "127.0.0.1:1234",
            "/key/"
        )
        .unwrap(),
        "index.html"
    );
    for req in [
        "GET /key/%2e%2e/secret HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n",
        "GET /key/ HTTP/1.1\r\nHost: attacker.example\r\n\r\n",
        "POST /key/ HTTP/1.1\r\nHost: 127.0.0.1:1234\r\n\r\n",
    ] {
        assert!(preview::request_path(req, "127.0.0.1:1234", "/key/").is_err());
    }
}
#[test]
fn recovery_does_not_modify_source_hash() {
    let (_t, s) = fixture();
    let s = ready(&s);
    protocol::execute(Request{schema:1,command:"recovery-document".into(),path:s.root.clone(),args:json!({"article":"content/today-in-omarchy/story.md","meta":{"title":"Unsaved"},"body":"Recovered words","expected_source_hash":s.hash})}).unwrap();
    assert_eq!(snapshot(&s.root).unwrap().hash, s.hash);
    let v = protocol::execute(Request {
        schema: 1,
        command: "read".into(),
        path: s.root,
        args: json!({"article":"content/today-in-omarchy/story.md"}),
    })
    .unwrap();
    assert_eq!(v["recovery"]["body"], "Recovered words");
}
#[test]
fn media_import_normalises_and_checks_content() {
    let (t, s) = fixture();
    let media = t.path().join("strange name.jpg");
    fs::write(&media, b"\x89PNG\r\n\x1a\nfixture").unwrap();
    let (s, p) = import_media(&s.root, &media, &s.hash).unwrap();
    assert!(p.ends_with(".png"));
    assert!(!p.contains(' '));
    fs::write(&media, b"#!/bin/sh\necho bad").unwrap();
    assert!(import_media(&s.root, &media, &s.hash).is_err());
}
#[test]
fn public_output_manifest_detects_tampering() {
    let (_t, s) = fixture();
    let s = ready(&s);
    let b = render::build(&s, &state(&s.root).unwrap(), false).unwrap();
    render::write_build(&s.root, std::path::Path::new("output"), &b).unwrap();
    fs::write(s.root.join("output/index.html"), "tampered").unwrap();
    assert!(
        protocol::execute(Request {
            schema: 1,
            command: "feed-check".into(),
            path: s.root,
            args: json!({})
        })
        .is_err()
    );
}
#[test]
fn external_images_never_load_in_offline_preview() {
    let (_t, s) = fixture();
    let html = render::markdown(
        "![Remote alt](https://example.com/image.png)",
        &s.config,
        true,
    );
    assert!(!html.contains("<img"));
    assert!(html.contains("Remote alt"));
}
#[test]
fn empty_publication_has_valid_feeds() {
    let (_t, s) = fixture();
    let b = render::build(&s, &state(&s.root).unwrap(), false).unwrap();
    render::feed_check_files(&b.files).unwrap();
    let feed: serde_json::Value = serde_json::from_slice(&b.files["feed.json"]).unwrap();
    assert_eq!(feed["items"], json!([]));
}

#[test]
fn x_preview_is_offline_but_clipboard_retains_image_reference() {
    let (_t, s) = fixture();
    let a = parse_article(
        "content/a.md",
        article(
            ID,
            "story",
            Status::Draft,
            "![Remote](https://example.com/image.png)",
        )
        .as_bytes(),
    )
    .unwrap();
    let export = render::export_x(&s, &a);
    assert!(!export.preview_html.contains("<img"));
    assert!(
        export
            .html
            .contains("src=\"https://example.com/image.png\"")
    );
}
