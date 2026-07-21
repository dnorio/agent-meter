use axum::{extract::State, routing::get, Json, Router};
use serde_json::{json, Value};

use crate::app::AppState;

async fn version_handler(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "service": "agent-meter-collector",
        "version": env!("CARGO_PKG_VERSION"),
        "require_api_key": state.config.require_api_key,
        "openapi": "/openapi.json",
        "docs": "/docs",
    }))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/version", get(version_handler))
}
