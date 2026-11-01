//! Signed, expiring browser capabilities. Caller-selected scenario IDs are never trusted.
use crate::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use ed25519_dalek::{Signer, Verifier};
use rand_core::{OsRng, RngCore};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_secs()
}
pub fn mint(key: &ed25519_dalek::SigningKey, id: &str, expiry: u64) -> String {
    let body = format!("{id}.{expiry}");
    let signature = key.sign(body.as_bytes());
    format!("{body}.{}", hex::encode(signature.to_bytes()))
}
fn inspect(key: &ed25519_dalek::SigningKey, token: &str) -> Option<String> {
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 || parts[0].len() != 64 || !parts[0].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    let expiry: u64 = parts[1].parse().ok()?;
    if expiry <= now() || expiry > now() + 86400 {
        return None;
    }
    let signature = ed25519_dalek::Signature::from_slice(&hex::decode(parts[2]).ok()?).ok()?;
    key.verifying_key()
        .verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &signature)
        .ok()?;
    Some(parts[0].to_owned())
}
fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({"error":{"code":code,"message":message}})),
    )
        .into_response()
}
pub async fn protect(
    State(app): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let protected = request.uri().path().starts_with("/v1/");
    let peer = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|addr| addr.0.ip().to_string())
        .unwrap_or_else(|| "unknown".into());
    let cookie = request
        .headers()
        .get("cookie")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| {
            s.split(';')
                .find_map(|p| p.trim().strip_prefix("hr_session="))
        })
        .map(str::to_owned);
    request.headers_mut().remove("x-demo-session");
    let cookie = if request.uri().path() == "/v1/session/new" {
        None
    } else {
        cookie
    };
    let mut set_cookie = None;
    if protected {
        let id = match cookie {
            Some(token) => match inspect(&app.signing_key, &token) {
                Some(id) => id,
                None => {
                    return error(
                        StatusCode::UNAUTHORIZED,
                        "SESSION_EXPIRED",
                        "Your scenario session expired. Start a new session to continue.",
                    )
                }
            },
            None => {
                let mut nonce = [0u8; 32];
                OsRng.fill_bytes(&mut nonce);
                let id = hex::encode(nonce);
                let secure = if app.public_origin.starts_with("https://") {
                    "; Secure"
                } else {
                    ""
                };
                set_cookie = Some(format!(
                    "hr_session={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=86400{secure}",
                    mint(&app.signing_key, &id, now() + 86400)
                ));
                id
            }
        };
        {
            let mut limits = app.request_limits.lock().expect("limits lock");
            limits.retain(|_, (started, _)| now().saturating_sub(*started) < 60);
            let peer_key = format!("peer:{peer}");
            let peer_count = limits.entry(peer_key).or_insert((now(), 0));
            peer_count.1 += 1;
            if peer_count.1 > 300 {
                return error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "RATE_LIMITED",
                    "Too many requests from this network. Wait a minute and retry.",
                );
            }
            if limits.len() >= 10000 && !limits.contains_key(&id) {
                return error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "CAPACITY",
                    "Demo capacity reached. Try again later.",
                );
            }
            let entry = limits.entry(id.clone()).or_insert((now(), 0));
            entry.1 += 1;
            if entry.1 > 120 {
                return error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "RATE_LIMITED",
                    "Too many requests. Wait a minute and retry.",
                );
            }
        }
        request.headers_mut().insert(
            "x-demo-session",
            HeaderValue::from_str(&id).expect("hex id"),
        );
    }
    let mut response = next.run(request).await;
    if let Some(cookie) = set_cookie {
        response.headers_mut().insert(
            "set-cookie",
            HeaderValue::from_str(&cookie).expect("cookie"),
        );
    }
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("same-origin"));
    response
        .headers_mut()
        .insert("x-frame-options", HeaderValue::from_static("DENY"));
    response.headers_mut().insert("content-security-policy", HeaderValue::from_static("default-src 'self'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"));
    response
        .headers_mut()
        .insert("record-version", HeaderValue::from_static("1"));
    let mut request_id = [0u8; 16];
    OsRng.fill_bytes(&mut request_id);
    response.headers_mut().insert(
        "x-request-id",
        HeaderValue::from_str(&hex::encode(request_id)).expect("request id"),
    );
    if app.public_origin.starts_with("https://") {
        response.headers_mut().insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=31536000"),
        );
    }
    if protected {
        if (response.status().is_client_error() || response.status().is_server_error())
            && !response.headers().contains_key("content-type")
        {
            let status = response.status();
            *response.body_mut() = Body::from(serde_json::json!({"error":{"code":status.as_str(),"message":status.canonical_reason().unwrap_or("Request failed")}}).to_string());
            response
                .headers_mut()
                .insert("content-type", HeaderValue::from_static("application/json"));
        }
        response
            .headers_mut()
            .insert("cache-control", HeaderValue::from_static("no-store"));
    }
    response
}

pub fn load_key() -> ed25519_dalek::SigningKey {
    use std::os::unix::fs::OpenOptionsExt;
    use std::{
        fs::{self, OpenOptions},
        io::Write,
    };
    let path = std::env::var("SIGNING_KEY_FILE").unwrap_or_else(|_| ".local/signing.key".into());
    if let Ok(bytes) = fs::read(&path) {
        return ed25519_dalek::SigningKey::from_bytes(
            &bytes.try_into().expect("signing key must contain 32 bytes"),
        );
    }
    if let Some(parent) = std::path::Path::new(&path).parent() {
        fs::create_dir_all(parent).expect("key directory");
    }
    let key = ed25519_dalek::SigningKey::generate(&mut OsRng);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .expect("create signing key file; configure SIGNING_KEY_FILE for deployment");
    file.write_all(&key.to_bytes()).expect("persist key");
    key
}
