use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::errors::AppError;
use crate::services::report_service::{self, ReportQuery};

#[derive(Debug, Deserialize, Default)]
pub struct ReportParams {
    from: Option<String>,
    to: Option<String>,
    repo: Option<String>,
    ide: Option<String>,
    skill: Option<String>,
    limit: Option<i64>,
}

impl ReportParams {
    fn into_query(self) -> ReportQuery {
        let from = self
            .from
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));
        let to = self
            .to
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        ReportQuery {
            from,
            to,
            repo: self.repo,
            ide: self.ide,
            skill: self.skill,
            limit: self.limit,
        }
    }
}

async fn top_tools(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = report_service::top_tools(&state.pool, &params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn top_tasks(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = report_service::top_tasks(&state.pool, &params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn top_mcp_servers(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = report_service::top_mcp_servers(&state.pool, &params.into_query()).await?;
    Ok(Json(json!(results)))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/reports/top-tools", get(top_tools))
        .route("/reports/top-tasks", get(top_tasks))
        .route("/reports/top-mcp-servers", get(top_mcp_servers))
}
