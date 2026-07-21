use axum::{
    extract::{ConnectInfo, State},
    http::HeaderMap,
    routing::post,
    Json, Router,
};
use std::net::SocketAddr;

use crate::app::AppState;
use crate::errors::AppError;
use crate::models::event::ToolCallEvent;
use crate::services::{auth, ingest};

async fn post_tool_call(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(event): Json<ToolCallEvent>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth::authorize_ingest(state.db.as_ref(), &headers, state.config.require_api_key).await?;
    let client_ip = ingest::client_ip(&headers, Some(&addr.ip().to_string()));
    ingest::enqueue_tool_call(&state, event, &headers, &client_ip).await
}

pub fn router() -> Router<AppState> {
    Router::new().route("/events/tool-call", post(post_tool_call))
}
