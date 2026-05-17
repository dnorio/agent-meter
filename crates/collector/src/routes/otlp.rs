use axum::{extract::State, routing::post, Json, Router};
use serde_json::Value;

use crate::app::AppState;
use crate::errors::AppError;
use crate::otlp;

async fn post_traces(
    State(state): State<AppState>,
    body: axum::body::Bytes,
) -> Result<Json<Vec<Value>>, AppError> {
    let results = otlp::handle_trace_request(&body, &state.pool)?;
    Ok(Json(results))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/traces", post(post_traces))
}
