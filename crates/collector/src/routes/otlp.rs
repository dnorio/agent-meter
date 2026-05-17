use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use serde_json::Value;

use crate::app::AppState;
use crate::errors::AppError;
use crate::otlp;

async fn post_traces(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<Vec<Value>>, AppError> {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok());
    let results = otlp::handle_trace_request(&body, content_type, &state.pool)?;
    Ok(Json(results))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/traces", post(post_traces))
}
