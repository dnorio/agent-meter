use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::app::AppState;
use crate::errors::AppError;
use crate::services::search_service;

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    q: String,
    limit: Option<i64>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/search", get(search_handler))
}

async fn search_handler(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<search_service::SearchResult>>, AppError> {
    let q = params.q.trim();
    if q.is_empty() || q.len() < 2 {
        return Ok(Json(vec![]));
    }
    let limit = params.limit.unwrap_or(20).min(50);
    let results = search_service::search(&state.pool, q, limit).await?;
    Ok(Json(results))
}
