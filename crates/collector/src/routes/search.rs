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
    // A negative value is not a meaningful cap and would become `LIMIT -1`, which SQLite
    // treats as "no limit"; clamp unknown/invalid values to the documented default and cap.
    let limit = match params.limit {
        Some(n) if n <= 0 => 20,
        Some(n) => n.min(50),
        None => 20,
    };
    let results = state.db.search(q, limit).await?;
    Ok(Json(results))
}
