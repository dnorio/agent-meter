//! T-352 — Notification Channels service (CRUD + dispatch)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct NotificationChannel {
    pub id: Uuid,
    pub org_id: Option<Uuid>,
    pub name: String,
    pub kind: String,
    pub config: serde_json::Value,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChannel {
    pub name: String,
    pub kind: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateChannel {
    pub name: Option<String>,
    pub config: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct DispatchResult {
    pub channel_id: Uuid,
    pub channel_name: String,
    pub success: bool,
    pub error: Option<String>,
}

pub async fn list(pool: &PgPool) -> Result<Vec<NotificationChannel>, AppError> {
    let rows = sqlx::query_as::<_, NotificationChannel>(
        "SELECT id, org_id, name, kind, config, enabled, created_at \
         FROM notification_channels ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<NotificationChannel, AppError> {
    sqlx::query_as::<_, NotificationChannel>(
        "SELECT id, org_id, name, kind, config, enabled, created_at \
         FROM notification_channels WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("channel not found".into()))
}

pub async fn create(pool: &PgPool, input: CreateChannel) -> Result<NotificationChannel, AppError> {
    let row = sqlx::query_as::<_, NotificationChannel>(
        "INSERT INTO notification_channels (name, kind, config) \
         VALUES ($1, $2, $3) \
         RETURNING id, org_id, name, kind, config, enabled, created_at",
    )
    .bind(&input.name)
    .bind(&input.kind)
    .bind(&input.config)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    input: UpdateChannel,
) -> Result<NotificationChannel, AppError> {
    let current = get(pool, id).await?;
    let name = input.name.unwrap_or(current.name);
    let config = input.config.unwrap_or(current.config);
    let enabled = input.enabled.unwrap_or(current.enabled);

    let row = sqlx::query_as::<_, NotificationChannel>(
        "UPDATE notification_channels SET name=$1, config=$2, enabled=$3 \
         WHERE id=$4 \
         RETURNING id, org_id, name, kind, config, enabled, created_at",
    )
    .bind(&name)
    .bind(&config)
    .bind(enabled)
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM notification_channels WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("channel not found".into()));
    }
    Ok(())
}

/// Dispatch a notification to all enabled channels
pub async fn dispatch(
    pool: &PgPool,
    subject: &str,
    body: &str,
) -> Result<Vec<DispatchResult>, AppError> {
    let channels = list(pool).await?;
    let mut results = Vec::new();

    for ch in channels.iter().filter(|c| c.enabled) {
        let result = dispatch_single(ch, subject, body).await;
        results.push(result);
    }
    Ok(results)
}

async fn dispatch_single(
    channel: &NotificationChannel,
    subject: &str,
    body: &str,
) -> DispatchResult {
    let res = match channel.kind.as_str() {
        "webhook" => dispatch_webhook(channel, subject, body).await,
        "slack" => dispatch_slack(channel, subject, body).await,
        "email" => Ok(()), // email not yet implemented
        _ => Err(format!("unsupported channel kind: {}", channel.kind)),
    };

    DispatchResult {
        channel_id: channel.id,
        channel_name: channel.name.clone(),
        success: res.is_ok(),
        error: res.err(),
    }
}

async fn dispatch_webhook(
    channel: &NotificationChannel,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    let url = channel
        .config
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or("webhook config missing 'url'")?;

    let payload = serde_json::json!({
        "subject": subject,
        "body": body,
        "source": "agent-meter",
        "channel_id": channel.id.to_string(),
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("webhook request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("webhook returned {}", resp.status()));
    }
    Ok(())
}

async fn dispatch_slack(
    channel: &NotificationChannel,
    subject: &str,
    body: &str,
) -> Result<(), String> {
    let webhook_url = channel
        .config
        .get("webhook_url")
        .and_then(|v| v.as_str())
        .ok_or("slack config missing 'webhook_url'")?;

    let payload = serde_json::json!({
        "text": format!("*{}*\n{}", subject, body),
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(webhook_url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("slack request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("slack returned {}", resp.status()));
    }
    Ok(())
}
