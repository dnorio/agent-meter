//! T-319 — Multi-tenant scaffolding (MVP).
//!
//! Lista organizations e gerencia API keys.
//! Auth gating não está habilitado por padrão — controle via env `REQUIRE_API_KEY=true`
//! (não implementado nesta MVP; previsto em T-319.1).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub plan: String,
    pub created_at: DateTime<Utc>,
}

pub async fn list_orgs(pool: &PgPool) -> Result<Vec<Organization>, AppError> {
    let orgs: Vec<Organization> = sqlx::query_as(
        "SELECT id, slug, name, plan, created_at FROM organizations ORDER BY created_at ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(orgs)
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ApiKey {
    pub id: Uuid,
    pub org_id: Uuid,
    pub key_prefix: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn list_api_keys(pool: &PgPool, org_id: Uuid) -> Result<Vec<ApiKey>, AppError> {
    let keys: Vec<ApiKey> = sqlx::query_as(
        r#"SELECT id, org_id, key_prefix, name, created_at, last_used_at, revoked_at
           FROM api_keys WHERE org_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(org_id)
    .fetch_all(pool)
    .await?;
    Ok(keys)
}

#[derive(Debug, Serialize)]
pub struct CreatedApiKey {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub key_prefix: String,
    /// Plain text — só retornado UMA vez, na criação.
    pub secret: String,
}

pub async fn create_api_key(
    pool: &PgPool,
    org_id: Uuid,
    name: &str,
) -> Result<CreatedApiKey, AppError> {
    // Gera segredo a partir de 2 UUIDs concatenados (32 bytes de entropia).
    let body = format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let secret = format!("am_live_{}", body);
    let key_prefix: String = secret.chars().take(16).collect();
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let key_hash = hex::encode(hasher.finalize());

    let id: Uuid = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO api_keys (id, org_id, key_prefix, key_hash, name)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(id)
    .bind(org_id)
    .bind(&key_prefix)
    .bind(&key_hash)
    .bind(name)
    .execute(pool)
    .await?;

    Ok(CreatedApiKey {
        id,
        org_id,
        name: name.to_string(),
        key_prefix,
        secret,
    })
}

pub async fn revoke_api_key(pool: &PgPool, key_id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE api_keys SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL")
        .bind(key_id)
        .execute(pool)
        .await?;
    Ok(())
}
