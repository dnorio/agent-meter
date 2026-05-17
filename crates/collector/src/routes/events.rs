use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use serde_json::{json, Value};

use crate::app::AppState;
use crate::errors::AppError;
use crate::models::event::ToolCallEvent;
use crate::services::event_service;

async fn post_tool_call(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut event): Json<ToolCallEvent>,
) -> Result<Json<Value>, AppError> {
    if event.client_ip.is_none() {
        event.client_ip = headers
            .get("x-forwarded-for")
            .or_else(|| headers.get("x-real-ip"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());
    }
    if event.user_agent.is_none() {
        event.user_agent = headers.get("user-agent").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
    }
    let record = event_service::insert_tool_call(&state.pool, event).await?;

    Ok(Json(json!({
        "event_id": record.event_id,
        "duration_ms": record.duration_ms,
        "estimated_total_tokens": record.estimated_total_tokens,
    })))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/events/tool-call", post(post_tool_call))
}
