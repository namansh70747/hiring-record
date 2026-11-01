//! Operator boundary. Production refuses the loopback development login.
use crate::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use ed25519_dalek::{Signer, Verifier};

pub fn dev_login_allowed(deployment_mode: &str, public_origin: &str) -> bool {
    deployment_mode != "production"
        && (public_origin.starts_with("http://127.0.0.1")
            || public_origin.starts_with("http://localhost"))
}

pub fn mint_operator(key: &ed25519_dalek::SigningKey, subject: &str, expiry: u64) -> String {
    let body = format!("operator.{subject}.{expiry}");
    let signature = key.sign(body.as_bytes());
    format!("{body}.{}", hex::encode(signature.to_bytes()))
}

pub fn inspect_operator(key: &ed25519_dalek::SigningKey, token: &str, now: u64) -> Option<String> {
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 4 || parts[0] != "operator" {
        return None;
    }
    let expiry: u64 = parts[2].parse().ok()?;
    if expiry <= now {
        return None;
    }
    let signature = ed25519_dalek::Signature::from_slice(&hex::decode(parts[3]).ok()?).ok()?;
    key.verifying_key()
        .verify(
            format!("operator.{}.{}", parts[1], parts[2]).as_bytes(),
            &signature,
        )
        .ok()?;
    Some(parts[1].to_owned())
}

pub async fn login(State(app): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let mode = std::env::var("DEPLOYMENT_MODE").unwrap_or_else(|_| "local".into());
    if !dev_login_allowed(&mode, &app.public_origin) {
        return Err(StatusCode::FORBIDDEN);
    }
    let token = mint_operator(
        &app.session_key,
        "local-operator",
        crate::security::now() + 900,
    );
    Ok(Json(serde_json::json!({
        "operator": "local-operator",
        "expires_in": 900,
        "cookie": format!("hr_operator={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=900")
    })))
}

pub async fn status(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let token = headers
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .and_then(|cookie| {
            cookie
                .split(';')
                .find_map(|part| part.trim().strip_prefix("hr_operator="))
        })
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
        });
    let Some(token) = token else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    if inspect_operator(&app.session_key, token, crate::security::now()).is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(serde_json::json!({
        "synthetic": true,
        "ready": app.ready().await,
        "deployment_mode": std::env::var("DEPLOYMENT_MODE").unwrap_or_else(|_| "local".into()),
        "public_origin": app.public_origin,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_and_public_origins_refuse_dev_login() {
        assert!(dev_login_allowed("local", "http://127.0.0.1:8080"));
        assert!(!dev_login_allowed("production", "http://127.0.0.1:8080"));
        assert!(!dev_login_allowed("local", "https://hiring.example"));
    }

    #[test]
    fn operator_cookie_rejects_a_wrong_key() {
        let key = ed25519_dalek::SigningKey::from_bytes(&[3; 32]);
        let other = ed25519_dalek::SigningKey::from_bytes(&[4; 32]);
        let token = mint_operator(&key, "local-operator", 10_000);
        assert_eq!(
            inspect_operator(&key, &token, 1_000).as_deref(),
            Some("local-operator")
        );
        assert!(inspect_operator(&other, &token, 1_000).is_none());
        assert!(inspect_operator(&key, &token, 10_000).is_none());
    }
}
