//! T-322 — Status page, ToS, Privacy routes + status check endpoints

use axum::{
    extract::State,
    response::Html,
    routing::get,
    Json, Router,
};
use serde::Serialize;

use crate::app::AppState;
use crate::errors::AppError;

const STATUS_HTML: &str = include_str!("../../ui/status.html");
const TERMS_HTML: &str = include_str!("../../ui/terms.html");
const PRIVACY_HTML: &str = include_str!("../../ui/privacy.html");

// --- Pages ---

async fn status_page() -> Html<&'static str> {
    Html(STATUS_HTML)
}
async fn terms_page() -> Html<&'static str> {
    Html(TERMS_HTML)
}
async fn privacy_page() -> Html<&'static str> {
    Html(PRIVACY_HTML)
}

// --- Status API endpoints (consumed by /status UI) ---

#[derive(Serialize)]
struct StatusCheck {
    status: &'static str,
    latency_ms: Option<u64>,
}

async fn check_db(State(state): State<AppState>) -> Result<Json<StatusCheck>, AppError> {
    let start = std::time::Instant::now();
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await?;
    let ms = start.elapsed().as_millis() as u64;
    Ok(Json(StatusCheck {
        status: "ok",
        latency_ms: Some(ms),
    }))
}

async fn check_otlp() -> Json<StatusCheck> {
    // OTLP ingest is always available if the server is up (same binary)
    Json(StatusCheck {
        status: "ok",
        latency_ms: Some(0),
    })
}

async fn check_pricing(State(state): State<AppState>) -> Result<Json<StatusCheck>, AppError> {
    let start = std::time::Instant::now();
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::int8 FROM model_pricing")
            .fetch_one(&state.pool)
            .await?;
    let ms = start.elapsed().as_millis() as u64;
    let status = if count > 0 { "ok" } else { "degraded" };
    Ok(Json(StatusCheck {
        status,
        latency_ms: Some(ms),
    }))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(status_page))
        .route("/terms", get(terms_page))
        .route("/privacy", get(privacy_page))
        .route("/api/status/db", get(check_db))
        .route("/api/status/otlp", get(check_otlp))
        .route("/api/status/pricing", get(check_pricing))
}
