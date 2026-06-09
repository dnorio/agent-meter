use axum::{extract::{ConnectInfo, State}, http::HeaderMap, routing::post, Json, Router};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::net::SocketAddr;

use crate::app::AppState;
use crate::otlp;

async fn post_traces(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let content_type = headers.get("content-type").and_then(|v| v.to_str().ok());
    // X-Forwarded-For takes precedence (Ingress path); fall back to TCP socket addr
    let client_ip = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());

    // Rate limit check
    if state.rate_limiter.check(&client_ip).is_err() {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "60")],
            "rate limit exceeded",
        ).into_response();
    }

    let user_agent = headers.get("user-agent").and_then(|v| v.to_str().ok());
    match otlp::handle_trace_request(&body, content_type, Some(&client_ip), user_agent, &state.pool, state.ingest.as_ref()) {
        Ok(results) => Json(results).into_response(),
        Err(e) => e.into_response(),
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/traces", post(post_traces))
}
