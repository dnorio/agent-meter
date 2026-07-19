use axum::{
    extract::{Path, State},
    routing::{delete, post},
    Json, Router,
};
use serde_json::{json, Value};

use crate::app::AppState;
use crate::errors::AppError;

/// DELETE /api/conversations/{conversation_id} — remove one session and its events.
async fn delete_conversation(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let deleted = state.db.delete_conversation(&conversation_id).await?;
    Ok(Json(json!({
        "conversation_id": conversation_id,
        "deleted_events": deleted,
    })))
}

/// POST /api/admin/reset — wipe all ingested events (local SQLite reset).
async fn reset_all(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let deleted = state.db.reset_all_events().await?;
    Ok(Json(json!({
        "deleted_events": deleted,
        "message": "all events removed",
    })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/conversations/{conversation_id}",
            delete(delete_conversation),
        )
        .route("/api/admin/reset", post(reset_all))
}
