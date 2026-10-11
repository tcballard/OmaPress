use crate::{Error, Result};
use hmac::{Hmac, Mac};
use pulldown_cmark::{Event, Options, Parser, Tag};
use sha2::Sha256;
use uuid::Uuid;

pub fn email(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    let valid = value.len() <= 254
        && value.is_ascii()
        && !value
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || "<>(),;:\\\"".contains(c));
    let pieces: Vec<_> = value.split('@').collect();
    if !valid
        || pieces.len() != 2
        || pieces[0].is_empty()
        || !pieces[1].contains('.')
        || pieces[1].starts_with('.')
        || pieces[1].ends_with('.')
    {
        return Err(Error::bad("Use a plain email address"));
    }
    Ok(value)
}
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
pub fn token(key: &str, id: Uuid) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC accepts any key");
    mac.update(format!("unsubscribe:v1:{id}").as_bytes());
    format!("{id}.{}", hex::encode(mac.finalize().into_bytes()))
}
pub fn verify_token(key: &str, value: &str) -> Result<Uuid> {
    let (id, sig) = value
        .split_once('.')
        .ok_or_else(|| Error::bad("Invalid subscription link"))?;
    let id = Uuid::parse_str(id).map_err(|_| Error::bad("Invalid subscription link"))?;
    let signature = hex::decode(sig).map_err(|_| Error::bad("Invalid subscription link"))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC accepts any key");
    mac.update(format!("unsubscribe:v1:{id}").as_bytes());
    mac.verify_slice(&signature)
        .map_err(|_| Error::bad("Invalid subscription link"))?;
    Ok(id)
}
pub fn render(
    subject: &str,
    preview: &str,
    markdown: &str,
    name: &str,
    address: &str,
    unsubscribe: Option<&str>,
) -> Result<(String, String)> {
    if subject.trim().is_empty()
        || subject.len() > 200
        || subject.chars().any(char::is_control)
        || preview.len() > 300
        || markdown.len() > 80_000
    {
        return Err(Error::bad(
            "Subject, preview or article exceeds the email limits",
        ));
    }
    let mut text = String::new();
    let mut events = Vec::new();
    for event in Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    ) {
        match &event {
            Event::Html(_) | Event::InlineHtml(_) => {
                return Err(Error::bad("Raw HTML is not supported in newsletters"));
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let u = url::Url::parse(dest_url)
                    .map_err(|_| Error::bad("Newsletter images must be hosted HTTPS URLs"))?;
                if u.scheme() != "https"
                    || u.host_str().is_none()
                    || !u.username().is_empty()
                    || u.password().is_some()
                {
                    return Err(Error::bad("Newsletter images must be hosted HTTPS URLs"));
                }
                text.push_str(&format!(" [Image: {dest_url}] "));
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                if !dest_url.starts_with("https://")
                    && !dest_url.starts_with("http://")
                    && !dest_url.starts_with("mailto:")
                {
                    return Err(Error::bad(
                        "Email links must be absolute HTTP(S) or mailto URLs",
                    ));
                }
                text.push_str(&format!(" {dest_url} "));
            }
            Event::Text(t) | Event::Code(t) => text.push_str(t),
            Event::SoftBreak | Event::HardBreak | Event::End(_) => text.push('\n'),
            _ => {}
        }
        events.push(event);
    }
    let mut body = String::new();
    pulldown_cmark::html::push_html(&mut body, events.into_iter());
    let body = ammonia::clean(&body);
    let footer = unsubscribe
        .map(|u| {
            format!(
                "<p><a href=\"{}\">Unsubscribe from {}</a></p>",
                esc(u),
                esc(name)
            )
        })
        .unwrap_or("<p>Test message — not sent to subscribers.</p>".into());
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"></head><body><div style=\"display:none\">{}</div><main style=\"max-width:640px;margin:auto;font-family:sans-serif;line-height:1.6\"><h1>{}</h1>{body}<hr><p>{}<br>{}</p>{footer}</main></body></html>",
        esc(preview),
        esc(subject),
        esc(name),
        esc(address)
    );
    if html.len() > 100_000 {
        return Err(Error::bad("Rendered email exceeds 100 KB"));
    }
    let plain = format!(
        "{subject}\n\n{text}\n{name}\n{address}\n{}",
        unsubscribe.unwrap_or("Test message — not sent to subscribers.")
    );
    Ok((html, plain))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links_and_html_are_bounded() {
        assert!(render("Hello", "", "<script>alert(1)</script>", "N", "A", None).is_err());
        assert!(render("Hello", "", "![x](file:///secret)", "N", "A", None).is_err());
        let (html, plain) = render(
            "A & B",
            "preview",
            "**Hello**",
            "N",
            "Address",
            Some("https://example.com/u/token"),
        )
        .unwrap();
        assert!(html.contains("<strong>Hello</strong>"));
        assert!(plain.contains("https://example.com/u/token"));
        assert!(html.contains("A &amp; B"));
    }
    #[test]
    fn subscription_tokens_are_scoped() {
        let id = Uuid::new_v4();
        let t = token("key", id);
        assert_eq!(verify_token("key", &t).unwrap(), id);
        assert!(verify_token("other", &t).is_err());
        assert!(email("foo\r\nBcc: victim@example.com").is_err());
    }
}
