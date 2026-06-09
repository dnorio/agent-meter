//! T-318 — Cost Attribution Engine
//!
//! Calcula custo em USD via JOIN com `model_pricing` (função SQL `compute_event_usd`).
//! Casts NUMERIC para float8 no SQL para evitar dependência de bigdecimal.
//!
//! ## Billing Models (June 2026+)
//!
//! ALL chat/agent interactions are token-billed. No "free subscription" exists.
//!
//! - `copilot_credit`: GitHub Copilot AI Credits (1 credit = $0.01 USD).
//!   Plans: Pro ($15/mo = 1500 credits), Pro+ ($70), Max ($200).
//!   Tab completions are free/unlimited; chat/agent/CLI billed per token.
//!
//! - `cursor_usage`: Cursor usage-based (per-token, two pools: Auto+Composer & API).
//!   Plans: Pro ($20/mo includes $20 usage), Pro+ ($70/$70), Ultra ($400/$400).
//!   Tab completions are free/unlimited; chat/agent billed per token.
//!
//! - `token`: Direct API (Anthropic, OpenAI, etc.) — standard per-token pricing.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

use crate::errors::AppError;

#[derive(Debug, Serialize)]
pub struct CostKpis {
    pub total_usd: f64,
    pub total_credits: f64,
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
pub struct BillingModelBreakdown {
    pub billing_model: String,
    pub events: i64,
    pub usd_cost: f64,
    pub credits: f64,
}

#[derive(Debug, Serialize)]
pub struct CostSummary {
    pub kpis: CostKpis,
    pub by_model: Vec<ModelCost>,
    pub by_day: Vec<CostByDay>,
    pub by_billing_model: Vec<BillingModelBreakdown>,
}

pub async fn cost_summary(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    model: Option<&str>,
) -> Result<CostSummary, AppError> {
    // Run all 4 independent queries in parallel via tokio::join!
    #[derive(sqlx::FromRow)]
    struct KpiRow {
        total_usd: Option<f64>,
        total_credits: Option<f64>,
        total_events: Option<i64>,
        total_tokens_in: Option<i64>,
        total_tokens_out: Option<i64>,
        avg_duration_ms: Option<f64>,
        error_rate: Option<f64>,
    }

    #[derive(sqlx::FromRow)]
    struct ModelRow {
        model: Option<String>,
        events: i64,
        tokens_in: Option<i64>,
        tokens_out: Option<i64>,
        usd_cost: Option<f64>,
    }

    #[derive(sqlx::FromRow)]
    struct DayRow {
        day: DateTime<Utc>,
        usd_cost: Option<f64>,
        events: i64,
    }

    #[derive(sqlx::FromRow)]
    struct BillingRow {
        billing_model: String,
        events: i64,
        usd_cost: Option<f64>,
        credits: Option<f64>,
    }

    let kpi_fut = sqlx::query_as::<_, KpiRow>(
        r#"
        SELECT
            COALESCE(SUM(usd_cost), 0)::float8 AS total_usd,
            COALESCE(SUM(CASE WHEN billing_model = 'copilot_credit' THEN usd_cost * 100 ELSE 0 END), 0)::float8 AS total_credits,
            COUNT(*)::bigint AS total_events,
            SUM(estimated_input_tokens)::bigint AS total_tokens_in,
            SUM(estimated_output_tokens)::bigint AS total_tokens_out,
            AVG(duration_ms)::float8 AS avg_duration_ms,
            (COUNT(*) FILTER (WHERE NOT ok))::float8 / NULLIF(COUNT(*)::float8, 0) AS error_rate
        FROM agent_tool_calls
        WHERE started_at >= $1 AND started_at < $2
          AND ($3::text IS NULL OR model = $3)
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(model)
    .fetch_one(pool);

    let model_fut = sqlx::query_as::<_, ModelRow>(
        r#"
        SELECT
            model,
            COUNT(*)::bigint AS events,
            SUM(estimated_input_tokens)::bigint AS tokens_in,
            SUM(estimated_output_tokens)::bigint AS tokens_out,
            COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost
        FROM agent_tool_calls
        WHERE started_at >= $1 AND started_at < $2
          AND ($3::text IS NULL OR model = $3)
        GROUP BY model
        ORDER BY usd_cost DESC NULLS LAST
        LIMIT 50
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(model)
    .fetch_all(pool);

    let day_fut = sqlx::query_as::<_, DayRow>(
        r#"
        SELECT
            date_trunc('day', started_at) AS day,
            COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost,
            COUNT(*)::bigint AS events
        FROM agent_tool_calls
        WHERE started_at >= $1 AND started_at < $2
          AND ($3::text IS NULL OR model = $3)
        GROUP BY 1
        ORDER BY 1 ASC
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(model)
    .fetch_all(pool);

    let billing_fut = sqlx::query_as::<_, BillingRow>(
        r#"
        SELECT
            billing_model,
            COUNT(*)::bigint AS events,
            COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost,
            COALESCE(SUM(CASE WHEN billing_model = 'copilot_credit' THEN usd_cost * 100 ELSE 0 END), 0)::float8 AS credits
        FROM agent_tool_calls
        WHERE started_at >= $1 AND started_at < $2
          AND ($3::text IS NULL OR model = $3)
        GROUP BY billing_model
        ORDER BY events DESC
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(model)
    .fetch_all(pool);

    let (kpi_res, model_res, day_res, billing_res) =
        tokio::join!(kpi_fut, model_fut, day_fut, billing_fut);

    let kpi = kpi_res?;
    let by_model_raw = model_res?;
    let by_day_raw = day_res?;
    let by_billing_raw = billing_res?;

    let total_usd = kpi.total_usd.unwrap_or(0.0);
    let total_credits = kpi.total_credits.unwrap_or(0.0);
    let total_events = kpi.total_events.unwrap_or(0);

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

    let by_billing_model = by_billing_raw
        .into_iter()
        .map(|r| BillingModelBreakdown {
            billing_model: r.billing_model,
            events: r.events,
            usd_cost: r.usd_cost.unwrap_or(0.0),
            credits: r.credits.unwrap_or(0.0),
        })
        .collect();

    Ok(CostSummary {
        kpis: CostKpis {
            total_usd,
            total_credits,
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
        by_billing_model,
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
