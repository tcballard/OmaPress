use crate::{render, storage::*};
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::Path,
    time::Duration,
};

pub fn request_path(request: &str, host: &str, prefix: &str) -> Result<String> {
    let mut lines = request.split("\r\n");
    let line = lines.next().context("Missing request")?;
    let parts: Vec<_> = line.split_whitespace().collect();
    ensure!(
        parts.len() == 3 && ["GET", "HEAD"].contains(&parts[0]) && parts[2] == "HTTP/1.1",
        "Only HTTP/1.1 GET and HEAD are supported"
    );
    let hosts: Vec<_> = lines
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim())
        .collect();
    ensure!(hosts == [host], "Unexpected Host header");
    let raw = parts[1].split('?').next().unwrap_or("");
    let path = percent_encoding::percent_decode_str(raw).decode_utf8()?;
    let relative = path.strip_prefix(prefix).context("Unknown preview URL")?;
    let relative = if relative.is_empty() || relative.ends_with('/') {
        format!("{relative}index.html")
    } else {
        relative.into()
    };
    safe_relative(&relative)?;
    Ok(relative)
}
fn serve_one(mut stream: TcpStream, files: &BTreeMap<String, Vec<u8>>, host: &str, prefix: &str) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    while buffer.len() < 8192 && !buffer.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
        }
    }
    let request = String::from_utf8_lossy(&buffer);
    let path = request_path(&request, host, prefix);
    let (status, body, mime) = match path.ok().and_then(|p| files.get(&p).map(|b| (p, b))) {
        Some((p, b)) => {
            let mime = if p.ends_with(".html") {
                "text/html; charset=utf-8"
            } else if p.ends_with(".xml") {
                "application/xml; charset=utf-8"
            } else if p.ends_with(".json") {
                "application/json"
            } else if p.ends_with(".css") {
                "text/css"
            } else if p.ends_with(".svg") {
                "image/svg+xml"
            } else if p.ends_with(".png") {
                "image/png"
            } else if p.ends_with(".jpg") {
                "image/jpeg"
            } else if p.ends_with(".webp") {
                "image/webp"
            } else if p.ends_with(".gif") {
                "image/gif"
            } else {
                "text/plain"
            };
            ("200 OK", b.as_slice(), mime)
        }
        None => ("404 Not Found", b"Not found".as_slice(), "text/plain"),
    };
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Robots-Tag: noindex, nofollow\r\nContent-Security-Policy: default-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(headers.as_bytes());
    if !request.starts_with("HEAD ") {
        let _ = stream.write_all(body);
    }
}
pub fn run(root: &Path, bind: &str, port: u16, drafts: bool) -> Result<()> {
    ensure!(bind == "127.0.0.1", "Preview must bind to 127.0.0.1");
    let listener = TcpListener::bind((bind, port)).context("Cannot start loopback preview")?;
    listener.set_nonblocking(true)?;
    let host = listener.local_addr()?.to_string();
    let prefix = format!("/{}/", uuid::Uuid::new_v4());
    let mut s = snapshot(root)?;
    let ledger = state(root)?;
    let original = s.config.base_url.clone();
    let mut b = render::build(&s, &ledger, drafts)?;
    let base = format!("http://{host}{}", prefix.trim_end_matches('/'));
    // Rebase only textual output for local inspection. These bytes are private
    // preview artifacts; publish always rebuilds from the original snapshot.
    for (path, bytes) in &mut b.files {
        if path.ends_with(".html")
            || path.ends_with(".xml")
            || path.ends_with(".json")
            || path == "robots.txt"
        {
            *bytes = String::from_utf8(bytes.clone())?
                .replace(original.trim_end_matches('/'), &base)
                .into_bytes();
        }
    }
    s.config.base_url = base.clone();
    println!(
        "{}",
        serde_json::json!({"schema":1,"ok":true,"event":"preview_ready","url":format!("{base}/"),"source_hash":s.hash,"drafts":drafts})
    );
    std::io::stdout().flush()?;
    // Linux parent-death notification prevents orphan previews after desktop crashes.
    #[cfg(target_os = "linux")]
    unsafe {
        let parent = libc::getppid();
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
        if libc::getppid() != parent {
            return Ok(());
        }
    }
    loop {
        match listener.accept() {
            Ok((stream, _)) => serve_one(stream, &b.files, &host, &prefix),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(e) => return Err(e.into()),
        }
    }
}
