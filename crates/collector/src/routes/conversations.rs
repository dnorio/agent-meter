use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use agent_meter_db::models::ConversationRow;
use agent_meter_db::params::ConversationQuery;

use crate::app::AppState;
use crate::errors::AppError;

// ── HTML pages ──────────────────────────────────────────────────────────────
async fn page() -> impl IntoResponse {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        include_str!("../../ui/conversations.html"),
    )
}

async fn detail_page() -> impl IntoResponse {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        include_str!("../../ui/timeline.html"),
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

// ── Conversation timeline JSON (tool-call rows + summary) ────────────────────────
async fn get_timeline(
    State(state): State<AppState>,
    Path(conversation_id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let rows = state.db.conversation_detail(&conversation_id).await?;

    let event_count = rows.len();
    let started_at = rows.first().map(|r| r.started_at);
    let ended_at = rows.last().map(|r| r.ended_at);
    let title = rows.iter().find_map(|r| r.user_prompt.clone());
    let total_tokens: i64 = rows
        .iter()
        .filter_map(|r| r.estimated_total_tokens)
        .map(|t| t as i64)
        .sum();
    let total_tokens_in: i64 = rows
        .iter()
        .filter_map(|r| r.estimated_input_tokens)
        .map(|t| t as i64)
        .sum();
    let total_tokens_out: i64 = rows
        .iter()
        .filter_map(|r| r.estimated_output_tokens)
        .map(|t| t as i64)
        .sum();
    let total_usd_cost: f64 = rows.iter().filter_map(|r| r.usd_cost).sum();
    let total_duration_ms: i64 = rows.iter().map(|r| r.duration_ms as i64).sum();
    let error_count = rows.iter().filter(|r| !r.ok).count();

    // Expose `tokens_in`/`tokens_out` aliases per event so the timeline UI
    // (which expects those names) renders token counts correctly.
    let events: Vec<Value> = rows
        .iter()
        .map(|r| {
            let mut v = serde_json::to_value(r).unwrap_or(Value::Null);
            if let Value::Object(ref mut m) = v {
                m.insert("tokens_in".into(), json!(r.estimated_input_tokens));
                m.insert("tokens_out".into(), json!(r.estimated_output_tokens));
            }
            v
        })
        .collect();

    Ok(Json(json!({
        "conversation_id": conversation_id,
        "title": title,
        "event_count": event_count,
        "error_count": error_count,
        "started_at": started_at,
        "ended_at": ended_at,
        "total_tokens": total_tokens,
        "total_tokens_in": total_tokens_in,
        "total_tokens_out": total_tokens_out,
        "total_usd_cost": total_usd_cost,
        "total_duration_ms": total_duration_ms,
        "events": events,
    })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/conversations", get(page))
        .route("/conversations/{conversation_id}/timeline", get(detail_page))
        .route("/api/conversations", get(list))
        .route(
            "/api/conversations/{conversation_id}/timeline",
            get(get_timeline),
        )
}
