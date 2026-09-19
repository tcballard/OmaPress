//! Native public-client OAuth. Tokens belong to Secret Service, never publications.
use crate::process;
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant},
};
const REDIRECT: &str = "http://127.0.0.1:39123/callback";
const ATTRS: &[&str] = &["application", "pressroom", "service", "x"];
fn secret(operation: &str, input: &[u8]) -> Result<String> {
    let mut args = vec![operation];
    if operation == "store" {
        args.push("--label=Pressroom X account");
    }
    args.extend_from_slice(ATTRS);
    process::run_input("secret-tool", &args, None, Duration::from_secs(15), input).map_err(|_| {
        anyhow::anyhow!(
            "Secret Service unavailable or X is not connected. Unlock your keyring and connect X."
        )
    })
}
fn read() -> Result<Value> {
    let raw = secret("lookup", &[])?;
    // Migrate pre-OAuth manually stored tokens without copying them to disk.
    Ok(serde_json::from_str(raw.trim()).unwrap_or_else(|_| json!({"access_token":raw.trim()})))
}
fn store(value: &Value) -> Result<()> {
    secret("store", &serde_json::to_vec(value)?)?;
    Ok(())
}
pub fn disconnect() -> Result<Value> {
    let _lock = crate::storage::lock(&crate::substack::directory()?)?;
    secret("clear", &[])?;
    Ok(json!({"connected":false}))
}
pub fn status() -> Result<Value> {
    let v = read()?;
    Ok(
        json!({"connected":true,"username":v["username"],"expires_at":v["expires_at"],"refreshable":v["refresh_token"].is_string()}),
    )
}
fn exchange(fields: &[(&str, &str)]) -> Result<Value> {
    let form = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields.iter().copied())
        .finish();
    let out = process::run_input(
        "curl",
        &[
            "--disable",
            "--silent",
            "--fail",
            "--max-time",
            "30",
            "--proto",
            "=https",
            "--header",
            "Content-Type: application/x-www-form-urlencoded",
            "--data-binary",
            "@-",
            "https://api.x.com/2/oauth2/token",
        ],
        None,
        Duration::from_secs(35),
        form.as_bytes(),
    )
    .map_err(|_| {
        anyhow::anyhow!(
            "X authorization could not be completed. Reconnect X; no article was published."
        )
    })?;
    let mut v: Value = serde_json::from_str(&out).context("Invalid OAuth response")?;
    ensure!(
        v["access_token"].as_str().is_some_and(|s| !s.is_empty()),
        "X did not return an access token"
    );
    v["expires_at"] =
        json!(chrono::Utc::now().timestamp() + v["expires_in"].as_i64().unwrap_or(7200));
    Ok(v)
}
pub fn credentials() -> Result<Value> {
    let _lock = crate::storage::lock(&crate::substack::directory()?)?;
    let mut v = read()?;
    if v["expires_at"]
        .as_i64()
        .is_some_and(|t| t < chrono::Utc::now().timestamp() + 120)
    {
        let refresh = v["refresh_token"]
            .as_str()
            .context("X authorization expired. Reconnect X.")?;
        let client = v["client_id"]
            .as_str()
            .context("Reconnect X to refresh authorization")?;
        let mut new = exchange(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
            ("client_id", client),
        ])?;
        for key in ["client_id", "username", "user_id", "refresh_token"] {
            if new.get(key).is_none() {
                new[key] = v[key].clone();
            }
        }
        store(&new)?;
        v = new;
    }
    Ok(v)
}
fn base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = ((c[0] as u32) << 16)
            | ((c.get(1).copied().unwrap_or(0) as u32) << 8)
            | c.get(2).copied().unwrap_or(0) as u32;
        for i in 0..c.len() + 1 {
            out.push(TABLE[((n >> (18 - i * 6)) & 63) as usize] as char);
        }
    }
    out
}
fn random() -> Result<String> {
    let mut bytes = [0; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(base64url(&bytes))
}
fn callback(request: &str, state: &str) -> Result<String> {
    let line = request.lines().next().context("Empty callback")?;
    let parts: Vec<_> = line.split_whitespace().collect();
    ensure!(
        parts.len() == 3 && parts[0] == "GET" && parts[2] == "HTTP/1.1",
        "Invalid callback request"
    );
    let url = url::Url::parse(&format!("http://127.0.0.1:39123{}", parts[1]))?;
    ensure!(url.path() == "/callback", "Invalid callback path");
    ensure!(
        request
            .lines()
            .any(|line| line.eq_ignore_ascii_case("Host: 127.0.0.1:39123")),
        "Invalid callback host"
    );
    let params: Vec<_> = url.query_pairs().collect();
    let unique = |key: &str| -> Result<String> {
        let values: Vec<_> = params.iter().filter(|(k, _)| k == key).collect();
        ensure!(values.len() == 1, "Missing or duplicate OAuth parameter");
        Ok(values[0].1.to_string())
    };
    ensure!(unique("state")? == state, "OAuth state mismatch");
    ensure!(
        !params.iter().any(|(k, _)| k == "error"),
        "Authorization declined"
    );
    unique("code")
}
pub fn connect(client: &str) -> Result<Value> {
    let _lock = crate::storage::lock(&crate::substack::directory()?)?;
    ensure!(
        !client.is_empty()
            && client.len() < 1024
            && client
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b)),
        "Enter a valid public Native App client ID"
    );
    let listener = TcpListener::bind("127.0.0.1:39123")
        .context("Cannot open OAuth callback port 39123; close another sign-in and retry")?;
    listener.set_nonblocking(true)?;
    let verifier = random()?;
    let state = random()?;
    let challenge = base64url(&Sha256::digest(verifier.as_bytes()));
    let mut auth = url::Url::parse("https://x.com/i/oauth2/authorize")?;
    auth.query_pairs_mut().extend_pairs([
        ("response_type", "code"),
        ("client_id", client),
        ("redirect_uri", REDIRECT),
        (
            "scope",
            "tweet.read tweet.write users.read offline.access media.write",
        ),
        ("state", &state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ]);
    process::run("xdg-open", &[auth.as_str()], None, Duration::from_secs(10))
        .context("Cannot open your browser for X sign-in")?;
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline {
        let (mut stream, _) = match listener.accept() {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let mut bytes = Vec::new();
        while bytes.len() < 8192 && !bytes.ends_with(b"\r\n\r\n") {
            let mut b = [0];
            if stream.read(&mut b).unwrap_or(0) == 0 {
                break;
            }
            bytes.push(b[0]);
        }
        let result = callback(std::str::from_utf8(&bytes).unwrap_or(""), &state);
        let (code, message) = if result.is_ok() {
            ("200 OK", "Authorization received. Return to Pressroom.")
        } else {
            (
                "400 Bad Request",
                "Invalid authorization callback. Return to Pressroom and retry sign-in.",
            )
        };
        let _ = write!(
            stream,
            "HTTP/1.1 {code}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{message}",
            message.len()
        );
        let Ok(code) = result else { continue };
        let mut token = exchange(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", REDIRECT),
            ("client_id", client),
            ("code_verifier", &verifier),
        ])?;
        token["client_id"] = json!(client);
        let access = token["access_token"]
            .as_str()
            .context("Missing access token")?;
        ensure!(
            access
                .bytes()
                .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\'),
            "Invalid access token"
        );
        let config = format!("header = \"Authorization: Bearer {access}\"\n");
        let identity = process::run_input(
            "curl",
            &[
                "--disable",
                "--silent",
                "--fail",
                "--max-time",
                "30",
                "--proto",
                "=https",
                "--config",
                "-",
                "https://api.x.com/2/users/me",
            ],
            None,
            Duration::from_secs(35),
            config.as_bytes(),
        )
        .map_err(|_| anyhow::anyhow!("Could not verify the X account; reconnect X"))?;
        let identity: Value =
            serde_json::from_str(&identity).context("Invalid X account response")?;
        let id = identity["data"]["id"]
            .as_str()
            .context("X returned no account identity")?;
        ensure!(
            !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()),
            "Invalid X account identity"
        );
        token["user_id"] = json!(id);
        token["username"] = identity["data"]["username"].clone();
        store(&token)?;
        return status();
    }
    bail!("X sign-in timed out. No account was changed; try Connect X again.")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_rfc7636() {
        assert_eq!(
            base64url(&Sha256::digest(
                b"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"
            )),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn callback_validation() {
        assert_eq!(
            callback(
                "GET /callback?code=abc&state=s HTTP/1.1\r\nHost: 127.0.0.1:39123\r\n\r\n",
                "s"
            )
            .unwrap(),
            "abc"
        );
        for path in [
            "/callback?code=a&state=wrong",
            "/callback?code=a&state=s&state=s",
            "/other?code=a&state=s",
            "/callback?error=denied&state=s",
        ] {
            assert!(
                callback(
                    &format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:39123\r\n\r\n"),
                    "s"
                )
                .is_err()
            );
        }
    }
}
