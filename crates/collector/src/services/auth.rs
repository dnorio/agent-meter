//! API key validation for ingest endpoints (optional via `AGENT_METER_REQUIRE_API_KEY`).

use agent_meter_db::Database;
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

pub fn hash_key(token: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(token.as_bytes()))
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
    if meta.key_hash != hash_key(&token) {
        return Err(AppError::Unauthorized("invalid api key".into()));
    }
    Ok(Some(meta.org_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable() {
        assert_eq!(
            hash_key("am_live_testsecret"),
            hash_key("am_live_testsecret")
        );
    }

    #[test]
    fn prefix_length() {
        assert_eq!(key_prefix("am_live_abcd"), Some("am_live_abcd"));
        assert_eq!(key_prefix("short"), None);
    }
}
