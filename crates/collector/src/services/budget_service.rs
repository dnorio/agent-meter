//! T-351 — Budget CRUD + evaluation service

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Budget {
    pub id: Uuid,
    pub org_id: Option<Uuid>,
    pub name: String,
    pub period: String,
    pub amount_usd: f64,
    pub soft_threshold_pct: f64,
    pub hard_threshold_pct: f64,
    pub hard_cap: bool,
    pub filters: serde_json::Value,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBudget {
    pub name: String,
    pub period: Option<String>,
    pub amount_usd: f64,
    pub soft_threshold_pct: Option<f64>,
    pub hard_threshold_pct: Option<f64>,
    pub hard_cap: Option<bool>,
    pub filters: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBudget {
    pub name: Option<String>,
    pub period: Option<String>,
    pub amount_usd: Option<f64>,
    pub soft_threshold_pct: Option<f64>,
    pub hard_threshold_pct: Option<f64>,
    pub hard_cap: Option<bool>,
    pub filters: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BudgetStatus {
    pub budget: Budget,
    pub spent_usd: f64,
    pub pct_used: f64,
    pub breached_soft: bool,
    pub breached_hard: bool,
}

pub async fn list(pool: &PgPool) -> Result<Vec<Budget>, AppError> {
    let rows = sqlx::query_as::<_, Budget>(
        "SELECT id, org_id, name, period, amount_usd::float8 AS amount_usd, \
         soft_threshold_pct::float8 AS soft_threshold_pct, \
         hard_threshold_pct::float8 AS hard_threshold_pct, \
         hard_cap, filters, enabled, created_at \
         FROM budgets ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Budget, AppError> {
    sqlx::query_as::<_, Budget>(
        "SELECT id, org_id, name, period, amount_usd::float8 AS amount_usd, \
         soft_threshold_pct::float8 AS soft_threshold_pct, \
         hard_threshold_pct::float8 AS hard_threshold_pct, \
         hard_cap, filters, enabled, created_at \
         FROM budgets WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound("budget not found".into()))
}

pub async fn create(pool: &PgPool, input: CreateBudget) -> Result<Budget, AppError> {
    let period = input.period.unwrap_or_else(|| "monthly".into());
    let soft = input.soft_threshold_pct.unwrap_or(80.0);
    let hard = input.hard_threshold_pct.unwrap_or(100.0);
    let cap = input.hard_cap.unwrap_or(false);
    let filters = input.filters.unwrap_or(serde_json::json!({}));

    let row = sqlx::query_as::<_, Budget>(
        "INSERT INTO budgets (name, period, amount_usd, soft_threshold_pct, hard_threshold_pct, hard_cap, filters) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
         RETURNING id, org_id, name, period, amount_usd::float8 AS amount_usd, \
         soft_threshold_pct::float8 AS soft_threshold_pct, \
         hard_threshold_pct::float8 AS hard_threshold_pct, \
         hard_cap, filters, enabled, created_at",
    )
    .bind(&input.name)
    .bind(&period)
    .bind(input.amount_usd)
    .bind(soft)
    .bind(hard)
    .bind(cap)
    .bind(&filters)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn update(pool: &PgPool, id: Uuid, input: UpdateBudget) -> Result<Budget, AppError> {
    // Fetch current first
    let current = get(pool, id).await?;

    let name = input.name.unwrap_or(current.name);
    let period = input.period.unwrap_or(current.period);
    let amount = input.amount_usd.unwrap_or(current.amount_usd);
    let soft = input.soft_threshold_pct.unwrap_or(current.soft_threshold_pct);
    let hard = input.hard_threshold_pct.unwrap_or(current.hard_threshold_pct);
    let cap = input.hard_cap.unwrap_or(current.hard_cap);
    let filters = input.filters.unwrap_or(current.filters);
    let enabled = input.enabled.unwrap_or(current.enabled);

    let row = sqlx::query_as::<_, Budget>(
        "UPDATE budgets SET name=$1, period=$2, amount_usd=$3, soft_threshold_pct=$4, \
         hard_threshold_pct=$5, hard_cap=$6, filters=$7, enabled=$8 \
         WHERE id=$9 \
         RETURNING id, org_id, name, period, amount_usd::float8 AS amount_usd, \
         soft_threshold_pct::float8 AS soft_threshold_pct, \
         hard_threshold_pct::float8 AS hard_threshold_pct, \
         hard_cap, filters, enabled, created_at",
    )
    .bind(&name)
    .bind(&period)
    .bind(amount)
    .bind(soft)
    .bind(hard)
    .bind(cap)
    .bind(&filters)
    .bind(enabled)
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM budgets WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("budget not found".into()));
    }
    Ok(())
}

pub async fn evaluate_all(pool: &PgPool) -> Result<Vec<BudgetStatus>, AppError> {
    let budgets = list(pool).await?;
    let mut statuses = Vec::with_capacity(budgets.len());

    for budget in budgets {
        if !budget.enabled {
            continue;
        }
        let spent = compute_period_spend(pool, &budget).await?;
        let pct = if budget.amount_usd > 0.0 {
            (spent / budget.amount_usd) * 100.0
        } else {
            0.0
        };
        statuses.push(BudgetStatus {
            breached_soft: pct >= budget.soft_threshold_pct,
            breached_hard: pct >= budget.hard_threshold_pct,
            spent_usd: spent,
            pct_used: pct,
            budget,
        });
    }
    Ok(statuses)
}

async fn compute_period_spend(pool: &PgPool, budget: &Budget) -> Result<f64, AppError> {
    let interval = match budget.period.as_str() {
        "daily" => "1 day",
        "weekly" => "7 days",
        _ => "1 month",
    };

    let query = format!(
        "SELECT COALESCE(SUM(usd_cost), 0)::float8 AS total \
         FROM agent_tool_calls \
         WHERE started_at >= now() - interval '{}'",
        interval
    );

    let row: (f64,) = sqlx::query_as(&query).fetch_one(pool).await?;
    Ok(row.0)
}
