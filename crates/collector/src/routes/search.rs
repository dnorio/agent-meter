use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use agent_meter_db::models::SearchResultRow;

use crate::app::AppState;
use crate::errors::AppError;

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
) -> Result<Json<Vec<SearchResultRow>>, AppError> {
    let q = params.q.trim();
    if q.is_empty() || q.len() < 2 {
        return Ok(Json(vec![]));
    }
    let limit = params.limit.unwrap_or(20).min(50);
    let results = state.db.search(q, limit).await?;
    Ok(Json(results))
}
