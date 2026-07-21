//! Shared ingest path for REST and OTLP tool-call events.

use axum::http::HeaderMap;
use axum::Json;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::errors::AppError;
use crate::models::event::ToolCallEvent;
use crate::services::event_service;
use crate::services::ingest_buffer::TrySendEventError;

/// Resolve client IP from proxy headers or a direct connection fallback.
pub fn client_ip(headers: &HeaderMap, fallback: Option<&str>) -> String {
    headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .or_else(|| fallback.map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Fill optional telemetry fields from HTTP headers when absent on the event.
pub fn enrich_from_headers(event: &mut ToolCallEvent, headers: &HeaderMap) {
    if event.client_ip.is_none() {
        event.client_ip = headers
            .get("x-forwarded-for")
            .or_else(|| headers.get("x-real-ip"))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());
    }
    if event.user_agent.is_none() {
        event.user_agent = headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
    }
}

/// Rate-limit, buffer, and accept a REST tool-call event.
pub async fn enqueue_tool_call(
    state: &AppState,
    mut event: ToolCallEvent,
    headers: &HeaderMap,
    client_ip: &str,
) -> Result<Json<Value>, AppError> {
    if state.rate_limiter.check(client_ip).is_err() {
        return Err(AppError::TooManyRequests);
    }

    enrich_from_headers(&mut event, headers);

    let insert = event_service::to_insert(event.clone());
    let buffer = state
        .ingest
        .as_ref()
        .ok_or_else(|| AppError::Internal("ingest buffer unavailable".into()))?;

    buffer.try_send(event).map_err(|e| match e {
        TrySendEventError::Full => AppError::ServiceUnavailable,
        TrySendEventError::Closed => AppError::Internal("ingest buffer closed".into()),
    })?;

    Ok(Json(json!({
        "event_id": insert.event_id,
        "duration_ms": insert.duration_ms,
        "estimated_total_tokens": insert.estimated_total_tokens,
        "accepted": true,
    })))
}
