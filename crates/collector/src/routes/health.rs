use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::app::AppState;

/// Lightweight liveness probe (always 200).
async fn liveness() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "agent-meter-collector"
    }))
}

/// Deep readiness probe — verifies DB connectivity and reports buffer health.
async fn readiness(State(state): State<AppState>) -> Json<Value> {
    let db_ok = state.db.health_check().await.is_ok();

    let buffer_capacity = state.ingest.as_ref().map(|b| b.capacity());
    let buffer_queued = state.ingest.as_ref().map(|b| b.queued());

    let status = if db_ok { "ok" } else { "degraded" };

    Json(json!({
        "status": status,
        "service": "agent-meter-collector",
        "checks": {
            "database": db_ok,
            "ingest_buffer": {
                "capacity": buffer_capacity,
                "queued": buffer_queued,
            }
        }
    }))
}

/// OTLP ingest buffer health for status page.
async fn status_otlp(State(state): State<AppState>) -> Json<Value> {
    let cap = state.ingest.as_ref().map(|b| b.capacity()).unwrap_or(0);
    let queued = state.ingest.as_ref().map(|b| b.queued()).unwrap_or(0);
    let pct_used = if cap > 0 {
        (queued as f64 / cap as f64 * 100.0).round() as u32
    } else {
        0
    };
    let status = if pct_used > 90 { "degraded" } else { "ok" };
    Json(json!({"status": status, "capacity": cap, "queued": queued, "pct_used": pct_used}))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(liveness))
        .route("/health/ready", get(readiness))
        .route("/api/status/otlp", get(status_otlp))
}
