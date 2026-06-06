use crate::errors::AppError;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub org_id: Option<Uuid>,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub github_login: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GithubUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GithubEmail {
    pub email: String,
    pub primary: bool,
    pub verified: bool,
}

#[derive(Debug, Deserialize)]
struct GithubTokenResp {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

pub fn hash_token(token: &str) -> String {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    hex::encode(h.finalize())
}

/// Generates a new opaque session token (returned to client as cookie).
pub fn new_session_token() -> String {
    let a = Uuid::new_v4().simple().to_string();
    let b = Uuid::new_v4().simple().to_string();
    format!("{a}{b}")
}

pub async fn exchange_github_code(
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
) -> Result<String, AppError> {
    let client = reqwest::Client::new();
    let resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("github token exchange: {e}")))?;
    let parsed: GithubTokenResp = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("github token parse: {e}")))?;
    if let Some(token) = parsed.access_token {
        Ok(token)
    } else {
        Err(AppError::Internal(format!(
            "github oauth error: {} ({})",
            parsed.error.unwrap_or_default(),
            parsed.error_description.unwrap_or_default()
        )))
    }
}

pub async fn fetch_github_user(access_token: &str) -> Result<(GithubUser, String), AppError> {
    let client = reqwest::Client::new();
    let user: GithubUser = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", "agent-meter")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("github user: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("github user parse: {e}")))?;

    let email = if let Some(e) = user.email.clone() {
        e
    } else {
        let emails: Vec<GithubEmail> = client
            .get("https://api.github.com/user/emails")
            .header("Authorization", format!("Bearer {access_token}"))
            .header("User-Agent", "agent-meter")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("github emails: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("github emails parse: {e}")))?;
        emails
            .into_iter()
            .find(|e| e.primary && e.verified)
            .map(|e| e.email)
            .ok_or_else(|| AppError::Internal("no verified primary email on github".into()))?
    };

    Ok((user, email))
}

/// Upsert user (matched by github provider_id) and return user record + ensure
/// they have a personal organization (membership owner).
pub async fn upsert_github_user(
    pool: &PgPool,
    gh: &GithubUser,
    email: &str,
) -> Result<AuthUser, AppError> {
    // Upsert user
    let user_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (email, display_name, auth_provider, provider_id, avatar_url, github_login, last_login_at)
        VALUES ($1, $2, 'github', $3, $4, $5, now())
        ON CONFLICT (email) DO UPDATE
            SET display_name  = EXCLUDED.display_name,
                auth_provider = 'github',
                provider_id   = EXCLUDED.provider_id,
                avatar_url    = EXCLUDED.avatar_url,
                github_login  = EXCLUDED.github_login,
                last_login_at = now()
        RETURNING id
        "#,
    )
    .bind(email)
    .bind(gh.name.clone().unwrap_or_else(|| gh.login.clone()))
    .bind(gh.id.to_string())
    .bind(&gh.avatar_url)
    .bind(&gh.login)
    .fetch_one(pool)
    .await?;

    // Ensure personal org for this user
    let slug = format!("user-{}", gh.login.to_lowercase());
    let org_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO organizations (slug, name, plan)
        VALUES ($1, $2, 'free')
        ON CONFLICT (slug) DO UPDATE SET name = EXCLUDED.name
        RETURNING id
        "#,
    )
    .bind(&slug)
    .bind(gh.name.clone().unwrap_or_else(|| gh.login.clone()))
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO memberships (org_id, user_id, role)
        VALUES ($1, $2, 'owner')
        ON CONFLICT (org_id, user_id) DO NOTHING
        "#,
    )
    .bind(org_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(AuthUser {
        user_id,
        org_id: Some(org_id),
        email: email.to_string(),
        display_name: gh.name.clone(),
        avatar_url: gh.avatar_url.clone(),
        github_login: Some(gh.login.clone()),
    })
}

pub async fn create_session(
    pool: &PgPool,
    user_id: Uuid,
    org_id: Option<Uuid>,
    user_agent: Option<&str>,
    ip: Option<&str>,
) -> Result<String, AppError> {
    let token = new_session_token();
    let token_hash = hash_token(&token);
    let expires: DateTime<Utc> = Utc::now() + chrono::Duration::days(30);
    sqlx::query(
        r#"
        INSERT INTO sessions (token_hash, user_id, org_id, expires_at, user_agent, ip)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(&token_hash)
    .bind(user_id)
    .bind(org_id)
    .bind(expires)
    .bind(user_agent)
    .bind(ip)
    .execute(pool)
    .await?;
    Ok(token)
}

pub async fn delete_session(pool: &PgPool, token: &str) -> Result<(), AppError> {
    let h = hash_token(token);
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(h)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn lookup_session(pool: &PgPool, token: &str) -> Result<Option<AuthUser>, AppError> {
    let h = hash_token(token);
    #[derive(sqlx::FromRow)]
    struct Row {
        user_id: Uuid,
        org_id: Option<Uuid>,
        email: String,
        display_name: Option<String>,
        avatar_url: Option<String>,
        github_login: Option<String>,
    }
    let row: Option<Row> = sqlx::query_as(
        r#"
        SELECT s.user_id, s.org_id, u.email, u.display_name, u.avatar_url, u.github_login
        FROM sessions s
        JOIN users u ON u.id = s.user_id
        WHERE s.token_hash = $1 AND s.expires_at > now()
        "#,
    )
    .bind(&h)
    .fetch_optional(pool)
    .await?;
    match row {
        Some(r) => {
            // touch last_seen_at lazily
            let _ = sqlx::query("UPDATE sessions SET last_seen_at = now() WHERE token_hash = $1")
                .bind(&h)
                .execute(pool)
                .await;
            Ok(Some(AuthUser {
                user_id: r.user_id,
                org_id: r.org_id,
                email: r.email,
                display_name: r.display_name,
                avatar_url: r.avatar_url,
                github_login: r.github_login,
            }))
        }
        None => Ok(None),
    }
}

/// Lookup a user by API key (Bearer token in Authorization header).
/// Returns the AuthUser-like shape (no user_id; uses org owner).
pub async fn lookup_api_key(pool: &PgPool, raw_key: &str) -> Result<Option<Uuid>, AppError> {
    if !raw_key.starts_with("am_live_") {
        return Ok(None);
    }
    let key_hash = hash_token(raw_key);
    let org_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT org_id FROM api_keys
        WHERE key_hash = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(&key_hash)
    .fetch_optional(pool)
    .await?;
    if org_id.is_some() {
        let _ = sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE key_hash = $1")
            .bind(&key_hash)
            .execute(pool)
            .await;
    }
    Ok(org_id)
}

/// Generate a state token for OAuth CSRF protection.
/// Format: base64url(random_uuid + "|" + hmac_sha256(secret, uuid))
pub fn make_oauth_state(secret: &str) -> String {
    use hmac::{Hmac, Mac};
    let nonce = Uuid::new_v4().simple().to_string();
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(nonce.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!("{nonce}|{sig}"))
}

pub fn verify_oauth_state(secret: &str, state: &str) -> bool {
    use hmac::{Hmac, Mac};
    let decoded = match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(state) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let s = match String::from_utf8(decoded) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let mut parts = s.splitn(2, '|');
    let (nonce, sig) = match (parts.next(), parts.next()) {
        (Some(n), Some(s)) => (n, s),
        _ => return false,
    };
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(nonce.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    // constant-time compare of equal-length hex strings
    expected.len() == sig.len()
        && expected
            .bytes()
            .zip(sig.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}
