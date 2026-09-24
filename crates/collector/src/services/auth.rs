//! API key validation for ingest endpoints (optional via `AGENT_METER_REQUIRE_API_KEY`).

use agent_meter_db::Database;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::http::HeaderMap;
use uuid::Uuid;

use crate::errors::AppError;

pub fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

/// Hash an API key for at-rest storage (Argon2id — not a fast checksum).
pub fn hash_key(token: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(token.as_bytes(), &salt)
        .expect("argon2 hash")
        .to_string()
}

/// Verify a presented token against a stored Argon2 PHC string.
pub fn verify_key(token: &str, stored_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(token.as_bytes(), &parsed)
        .is_ok()
}

pub fn key_prefix(token: &str) -> Option<&str> {
    if token.len() >= 12 {
        Some(&token[..12])
    } else {
        None
    }
}

pub async fn authorize_ingest(
    db: &dyn Database,
    headers: &HeaderMap,
    required: bool,
) -> Result<Option<Uuid>, AppError> {
    let token = match extract_bearer(headers) {
        Some(t) => t,
        None if required => return Err(AppError::Unauthorized("missing api key".into())),
        None => return Ok(None),
    };

    let prefix =
        key_prefix(&token).ok_or_else(|| AppError::Unauthorized("invalid api key".into()))?;
    let meta = db
        .find_key_by_prefix(prefix)
        .await
        .map_err(AppError::from)?;
    let meta = meta.ok_or_else(|| AppError::Unauthorized("invalid api key".into()))?;
    if !verify_key(&token, &meta.key_hash) {
        return Err(AppError::Unauthorized("invalid api key".into()));
    }
    Ok(Some(meta.org_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_roundtrip_verifies() {
        let secret = "am_live_testsecret";
        let hashed = hash_key(secret);
        assert!(hashed.starts_with("$argon2"));
        assert!(verify_key(secret, &hashed));
        assert!(!verify_key("am_live_wrong", &hashed));
    }

    #[test]
    fn prefix_length() {
        assert_eq!(key_prefix("am_live_abcd"), Some("am_live_abcd"));
        assert_eq!(key_prefix("short"), None);
    }
}
