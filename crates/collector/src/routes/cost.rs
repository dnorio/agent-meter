//! T-318 — Rotas Cost.
//!
//! - `GET /api/cost/summary?from=&to=` — KPIs, by_model, by_day
//! - `GET /api/cost/pricing`           — lista pricing por modelo
//! - `GET /cost`                        — UI page

use axum::{
    extract::{Query, State},
    http::header,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use chrono::{Duration, Utc};
use serde::Deserialize;

use crate::app::AppState;
use crate::errors::AppError;
use crate::services::cost_service;

const COST_HTML: &str = include_str!("../../ui/cost.html");

#[derive(Debug, Deserialize, Default)]
pub struct CostParams {
    from: Option<String>,
    to: Option<String>,
    model: Option<String>,
}

async fn summary_handler(
    State(state): State<AppState>,
    Query(p): Query<CostParams>,
) -> Result<impl IntoResponse, AppError> {
    let to = p
        .to
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    let from = p
        .from
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| to - Duration::days(30));

    let summary = cost_service::cost_summary(&state.pool, from, to, p.model.as_deref()).await?;
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=60")],
        Json(summary),
    ))
}

async fn pricing_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<cost_service::PricingRow>>, AppError> {
    let rows = cost_service::list_pricing(&state.pool).await?;
    Ok(Json(rows))
}

async fn page_handler() -> Html<&'static str> {
    Html(COST_HTML)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/cost/summary", get(summary_handler))
        .route("/api/cost/pricing", get(pricing_handler))
        .route("/cost", get(page_handler))
}
