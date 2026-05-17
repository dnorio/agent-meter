use axum::{routing::get, Json, Router};
use serde_json::{json, Value};

async fn handler() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "agent-meter-collector"
    }))
}

pub fn router() -> Router<crate::app::AppState> {
    Router::new().route("/health", get(handler))
}
