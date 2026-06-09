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
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();

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

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(liveness))
        .route("/health/ready", get(readiness))
}
