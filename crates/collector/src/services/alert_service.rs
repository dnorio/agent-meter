//! T-320 — Alerts & Budgets (MVP).
//!
//! Avalia regras (`alert_rules`) contra a janela mais recente e grava `alert_history`.
//! Sem CronJob/Slack — o evaluator é exposto via endpoint manual `POST /api/alerts/evaluate`.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AlertRule {
    pub id: Uuid,
    pub org_id: Option<Uuid>,
    pub name: String,
    pub rule_type: String,
    pub window_minutes: i32,
    pub threshold: f64,
    pub comparator: String,
    pub filters: serde_json::Value,
    pub enabled: bool,
    pub cooldown_minutes: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct NewAlertRule {
    pub name: String,
    pub rule_type: String,
    pub window_minutes: i32,
    pub threshold: f64,
    pub comparator: Option<String>,
    pub filters: Option<serde_json::Value>,
    pub cooldown_minutes: Option<i32>,
}

pub async fn list_rules(pool: &PgPool) -> Result<Vec<AlertRule>, AppError> {
    let rows: Vec<AlertRule> = sqlx::query_as(
        r#"SELECT id, org_id, name, rule_type, window_minutes,
                  threshold::float8 AS threshold,
                  comparator, filters, enabled, cooldown_minutes, created_at
           FROM alert_rules ORDER BY created_at DESC"#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn create_rule(pool: &PgPool, input: NewAlertRule) -> Result<AlertRule, AppError> {
    let id = Uuid::new_v4();
    let comparator = input.comparator.as_deref().unwrap_or(">").to_string();
    let cooldown = input.cooldown_minutes.unwrap_or(60);
    let filters = input.filters.unwrap_or_else(|| serde_json::json!({}));

    sqlx::query(
        r#"INSERT INTO alert_rules
            (id, name, rule_type, window_minutes, threshold, comparator, filters, cooldown_minutes)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(id)
    .bind(&input.name)
    .bind(&input.rule_type)
    .bind(input.window_minutes)
    .bind(input.threshold)
    .bind(&comparator)
    .bind(&filters)
    .bind(cooldown)
    .execute(pool)
    .await?;

    let row: AlertRule = sqlx::query_as(
        r#"SELECT id, org_id, name, rule_type, window_minutes,
                  threshold::float8 AS threshold,
                  comparator, filters, enabled, cooldown_minutes, created_at
           FROM alert_rules WHERE id = $1"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete_rule(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM alert_rules WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AlertEvent {
    pub id: i64,
    pub rule_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub fired_at: DateTime<Utc>,
    pub observed_value: f64,
    pub threshold: f64,
    pub severity: String,
    pub payload: serde_json::Value,
    pub notified: bool,
}

pub async fn list_history(pool: &PgPool, limit: i64) -> Result<Vec<AlertEvent>, AppError> {
    let rows: Vec<AlertEvent> = sqlx::query_as(
        r#"SELECT id, rule_id, org_id, fired_at,
                  observed_value::float8 AS observed_value,
                  threshold::float8 AS threshold,
                  severity, payload, notified
           FROM alert_history ORDER BY fired_at DESC LIMIT $1"#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug, Serialize)]
pub struct EvaluateReport {
    pub rules_evaluated: usize,
    pub fired: Vec<FiredAlert>,
}

#[derive(Debug, Serialize)]
pub struct FiredAlert {
    pub rule_id: Uuid,
    pub rule_name: String,
    pub observed_value: f64,
    pub threshold: f64,
}

pub async fn evaluate(pool: &PgPool) -> Result<EvaluateReport, AppError> {
    let rules: Vec<AlertRule> = sqlx::query_as(
        r#"SELECT id, org_id, name, rule_type, window_minutes,
                  threshold::float8 AS threshold,
                  comparator, filters, enabled, cooldown_minutes, created_at
           FROM alert_rules WHERE enabled = true"#,
    )
    .fetch_all(pool)
    .await?;

    let mut fired = Vec::new();
    let now = Utc::now();

    for rule in &rules {
        let from = now - Duration::minutes(rule.window_minutes as i64);
        let observed = match rule.rule_type.as_str() {
            "cost_spike" => observe_cost(pool, from, now).await?,
            "error_rate" => observe_error_rate(pool, from, now).await?,
            "latency_p95" => observe_latency_p95(pool, from, now).await?,
            "token_burn" => observe_tokens(pool, from, now).await?,
            "tool_failure" => observe_tool_failures(pool, from, now).await?,
            _ => continue,
        };

        let breached = match rule.comparator.as_str() {
            ">" => observed > rule.threshold,
            ">=" => observed >= rule.threshold,
            "<" => observed < rule.threshold,
            "<=" => observed <= rule.threshold,
            _ => false,
        };

        if !breached {
            continue;
        }

        // Cooldown: ignora se já firou nos últimos `cooldown_minutes`
        let cd_from = now - Duration::minutes(rule.cooldown_minutes as i64);
        let recent: Option<i64> = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM alert_history WHERE rule_id = $1 AND fired_at >= $2",
        )
        .bind(rule.id)
        .bind(cd_from)
        .fetch_one(pool)
        .await?;

        if recent.unwrap_or(0) > 0 {
            continue;
        }

        let severity = if observed > rule.threshold * 2.0 {
            "critical"
        } else {
            "warning"
        };

        sqlx::query(
            r#"INSERT INTO alert_history
                (rule_id, org_id, observed_value, threshold, severity, payload, notified)
               VALUES ($1, $2, $3, $4, $5, $6, false)"#,
        )
        .bind(rule.id)
        .bind(rule.org_id)
        .bind(observed)
        .bind(rule.threshold)
        .bind(severity)
        .bind(serde_json::json!({"rule_type": rule.rule_type, "window_minutes": rule.window_minutes}))
        .execute(pool)
        .await?;

        fired.push(FiredAlert {
            rule_id: rule.id,
            rule_name: rule.name.clone(),
            observed_value: observed,
            threshold: rule.threshold,
        });
    }

    Ok(EvaluateReport {
        rules_evaluated: rules.len(),
        fired,
    })
}

async fn observe_cost(pool: &PgPool, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<f64, AppError> {
    let v: Option<f64> = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(usd_cost), 0)::float8
           FROM agent_tool_calls WHERE started_at >= $1 AND started_at < $2"#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    Ok(v.unwrap_or(0.0))
}

async fn observe_error_rate(pool: &PgPool, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<f64, AppError> {
    let v: Option<f64> = sqlx::query_scalar(
        r#"SELECT (COUNT(*) FILTER (WHERE NOT ok))::float8 / NULLIF(COUNT(*), 0)::float8 * 100.0
           FROM agent_tool_calls WHERE started_at >= $1 AND started_at < $2"#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    Ok(v.unwrap_or(0.0))
}

async fn observe_latency_p95(pool: &PgPool, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<f64, AppError> {
    let v: Option<f64> = sqlx::query_scalar(
        r#"SELECT COALESCE(percentile_disc(0.95) WITHIN GROUP (ORDER BY duration_ms), 0)::float8
           FROM agent_tool_calls WHERE started_at >= $1 AND started_at < $2"#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    Ok(v.unwrap_or(0.0))
}

async fn observe_tokens(pool: &PgPool, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<f64, AppError> {
    let v: Option<i64> = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(estimated_input_tokens + estimated_output_tokens), 0)::bigint
           FROM agent_tool_calls WHERE started_at >= $1 AND started_at < $2"#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    Ok(v.unwrap_or(0) as f64)
}

async fn observe_tool_failures(pool: &PgPool, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<f64, AppError> {
    let v: Option<i64> = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM agent_tool_calls WHERE NOT ok AND started_at >= $1 AND started_at < $2"#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;
    Ok(v.unwrap_or(0) as f64)
}
