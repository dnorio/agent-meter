use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};

use crate::app::AppState;
use crate::errors::AppError;
use crate::models::event::ToolCallEvent;
use crate::services::event_service;

async fn post_tool_call(
    State(state): State<AppState>,
    Json(event): Json<ToolCallEvent>,
) -> Result<Json<Value>, AppError> {
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
