//! T-318 — Cost Attribution Engine
//!
//! Calcula custo em USD via JOIN com `model_pricing` (função SQL `compute_event_usd`).
//! Casts NUMERIC para float8 no SQL para evitar dependência de bigdecimal.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

use crate::errors::AppError;

#[derive(Debug, Serialize)]
pub struct CostKpis {
    pub total_usd: f64,
    pub total_events: i64,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub avg_usd_per_event: f64,
    pub burn_rate_usd_per_hour: f64,
    pub avg_duration_ms: f64,
    pub error_rate: f64,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ModelCost {
    pub model: Option<String>,
    pub events: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub usd_cost: f64,
}

#[derive(Debug, Serialize)]
pub struct CostByDay {
    pub day: DateTime<Utc>,
    pub usd_cost: f64,
    pub events: i64,
}

#[derive(Debug, Serialize)]
pub struct CostSummary {
    pub kpis: CostKpis,
    pub by_model: Vec<ModelCost>,
    pub by_day: Vec<CostByDay>,
}

pub async fn cost_summary(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<CostSummary, AppError> {
    #[derive(sqlx::FromRow)]
    struct KpiRow {
        total_usd: Option<f64>,
        total_events: Option<i64>,
        total_tokens_in: Option<i64>,
        total_tokens_out: Option<i64>,
        avg_duration_ms: Option<f64>,
        error_rate: Option<f64>,
    }

    let kpi: KpiRow = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(usd_cost), 0)::float8 AS total_usd,
            COUNT(*)::bigint AS total_events,
            SUM(estimated_input_tokens)::bigint AS total_tokens_in,
            SUM(estimated_output_tokens)::bigint AS total_tokens_out,
            AVG(duration_ms)::float8 AS avg_duration_ms,
            (COUNT(*) FILTER (WHERE NOT ok))::float8 / NULLIF(COUNT(*)::float8, 0) AS error_rate
        FROM agent_tool_calls
        WHERE started_at >= $1 AND started_at < $2
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await?;

    let total_usd = kpi.total_usd.unwrap_or(0.0);
    let total_events = kpi.total_events.unwrap_or(0);

    #[derive(sqlx::FromRow)]
    struct ModelRow {
        model: Option<String>,
        events: i64,
        tokens_in: Option<i64>,
        tokens_out: Option<i64>,
        usd_cost: Option<f64>,
    }

    let by_model_raw: Vec<ModelRow> = sqlx::query_as(
        r#"
        SELECT
            model,
            COUNT(*)::bigint AS events,
            SUM(estimated_input_tokens)::bigint AS tokens_in,
            SUM(estimated_output_tokens)::bigint AS tokens_out,
            COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost
        FROM agent_tool_calls
        WHERE started_at >= $1 AND started_at < $2
        GROUP BY model
        ORDER BY usd_cost DESC NULLS LAST
        LIMIT 50
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let by_model = by_model_raw
        .into_iter()
        .map(|r| ModelCost {
            model: r.model,
            events: r.events,
            tokens_in: r.tokens_in.unwrap_or(0),
            tokens_out: r.tokens_out.unwrap_or(0),
            usd_cost: r.usd_cost.unwrap_or(0.0),
        })
        .collect();

    #[derive(sqlx::FromRow)]
    struct DayRow {
        day: DateTime<Utc>,
        usd_cost: Option<f64>,
        events: i64,
    }

    let by_day_raw: Vec<DayRow> = sqlx::query_as(
        r#"
        SELECT
            date_trunc('day', started_at) AS day,
            COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost,
            COUNT(*)::bigint AS events
        FROM agent_tool_calls
        WHERE started_at >= $1 AND started_at < $2
        GROUP BY 1
        ORDER BY 1 ASC
        "#,
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await?;

    let by_day = by_day_raw
        .into_iter()
        .map(|r| CostByDay {
            day: r.day,
            usd_cost: r.usd_cost.unwrap_or(0.0),
            events: r.events,
        })
        .collect();

    let hours = ((to - from).num_seconds() as f64 / 3600.0).max(1.0);
    let burn_rate = total_usd / hours;
    let avg = if total_events > 0 { total_usd / total_events as f64 } else { 0.0 };

    Ok(CostSummary {
        kpis: CostKpis {
            total_usd,
            total_events,
            total_tokens_in: kpi.total_tokens_in.unwrap_or(0),
            total_tokens_out: kpi.total_tokens_out.unwrap_or(0),
            avg_usd_per_event: avg,
            burn_rate_usd_per_hour: burn_rate,
            avg_duration_ms: kpi.avg_duration_ms.unwrap_or(0.0),
            error_rate: kpi.error_rate.unwrap_or(0.0),
            from,
            to,
        },
        by_model,
        by_day,
    })
}

#[derive(Debug, Serialize)]
pub struct PricingRow {
    pub id: i64,
    pub model: String,
    pub match_kind: String,
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cached_per_mtok: f64,
    pub priority: i32,
    pub source: String,
}

pub async fn list_pricing(pool: &PgPool) -> Result<Vec<PricingRow>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        model: String,
        match_kind: String,
        input_per_mtok: f64,
        output_per_mtok: f64,
        cached_per_mtok: f64,
        priority: i32,
        source: String,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"SELECT
            id,
            model,
            match_kind,
            input_per_mtok::float8 AS input_per_mtok,
            output_per_mtok::float8 AS output_per_mtok,
            cached_per_mtok::float8 AS cached_per_mtok,
            priority,
            source
        FROM model_pricing
        ORDER BY priority DESC, model"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| PricingRow {
            id: r.id,
            model: r.model,
            match_kind: r.match_kind,
            input_per_mtok: r.input_per_mtok,
            output_per_mtok: r.output_per_mtok,
            cached_per_mtok: r.cached_per_mtok,
            priority: r.priority,
            source: r.source,
        })
        .collect())
}
