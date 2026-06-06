//! T-320 — Rotas Alerts.

use axum::{
    extract::{Path, Query, State},
    response::Html,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::AppError;
use crate::services::alert_service;

const ALERTS_HTML: &str = include_str!("../../ui/alerts.html");

async fn list_rules_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<alert_service::AlertRule>>, AppError> {
    let rules = alert_service::list_rules(&state.pool).await?;
    Ok(Json(rules))
}

async fn create_rule_handler(
    State(state): State<AppState>,
    Json(body): Json<alert_service::NewAlertRule>,
) -> Result<Json<alert_service::AlertRule>, AppError> {
    let rule = alert_service::create_rule(&state.pool, body).await?;
    Ok(Json(rule))
}

async fn delete_rule_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    alert_service::delete_rule(&state.pool, id).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

#[derive(Debug, Deserialize)]
pub struct HistoryParams {
    limit: Option<i64>,
}

async fn history_handler(
    State(state): State<AppState>,
    Query(p): Query<HistoryParams>,
) -> Result<Json<Vec<alert_service::AlertEvent>>, AppError> {
    let limit = p.limit.unwrap_or(100).clamp(1, 1000);
    let h = alert_service::list_history(&state.pool, limit).await?;
    Ok(Json(h))
}

async fn evaluate_handler(
    State(state): State<AppState>,
) -> Result<Json<alert_service::EvaluateReport>, AppError> {
    let report = alert_service::evaluate(&state.pool).await?;
    Ok(Json(report))
}

async fn page_handler() -> Html<&'static str> {
    Html(ALERTS_HTML)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/alerts/rules", get(list_rules_handler).post(create_rule_handler))
        .route("/api/alerts/rules/:id", delete(delete_rule_handler))
        .route("/api/alerts/history", get(history_handler))
        .route("/api/alerts/evaluate", post(evaluate_handler))
        .route("/alerts", get(page_handler))
}
