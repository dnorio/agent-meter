use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use agent_meter_db::models::ConversationRow;
use agent_meter_db::params::ConversationQuery;

use crate::app::AppState;
use crate::errors::AppError;

// ── HTML page ─────────────────────────────────────────────────────────────────
async fn page() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8"),
         (header::CACHE_CONTROL, "no-store")],
        include_str!("../../ui/conversations.html"),
    )
}

// ── JSON API ──────────────────────────────────────────────────────────────────
#[derive(Deserialize)]
struct ListQuery {
    limit: Option<i64>,
    offset: Option<i64>,
    ide: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<ConversationRow>>, AppError> {
    let params = ConversationQuery {
        limit: q.limit.unwrap_or(50).min(200),
        offset: q.offset.unwrap_or(0),
        ide: q.ide,
    };
    let rows = state.db.list_conversations(&params).await?;
    Ok(Json(rows))
}

// ── Conversation detail JSON (tool-call rows) ───────────────────────────────────
async fn get_detail(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> Result<Json<Vec<agent_meter_db::models::ToolCallRow>>, AppError> {
    let rows = state.db.conversation_detail(&conversation_id).await?;
    Ok(Json(rows))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/conversations", get(page))
        .route("/api/conversations", get(list))
        .route("/api/conversations/:conversation_id/timeline", get(get_detail))
}
