use axum::{
    extract::{Query, State},
    response::Html,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::errors::AppError;
use crate::services::report_service::{self, EventQuery, ReportQuery};

const REPORTS_HTML: &str = include_str!("../../ui/reports.html");

async fn page() -> Html<&'static str> {
    Html(REPORTS_HTML)
}

#[derive(Debug, Deserialize, Default)]
pub struct ReportParams {
    from: Option<String>,
    to: Option<String>,
    repo: Option<String>,
    ide: Option<String>,
    agent: Option<String>,
    model: Option<String>,
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
            agent: self.agent,
            model: self.model,
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

#[derive(Debug, Deserialize, Default)]
pub struct CallsOverTimeParams {
    from: Option<String>,
    to: Option<String>,
    repo: Option<String>,
    ide: Option<String>,
    agent: Option<String>,
    model: Option<String>,
    skill: Option<String>,
    bucket: Option<String>,
}

async fn calls_over_time(
    State(state): State<AppState>,
    Query(params): Query<CallsOverTimeParams>,
) -> Result<Json<Value>, AppError> {
    let q = ReportParams {
        from: params.from,
        to: params.to,
        repo: params.repo,
        ide: params.ide,
        agent: params.agent,
        model: params.model,
        skill: params.skill,
        limit: None,
    }
    .into_query();
    let bucket = params.bucket.unwrap_or_else(|| "hour".into());
    let results = report_service::calls_over_time(&state.pool, &q, &bucket).await?;
    Ok(Json(json!(results)))
}

async fn by_ide(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = report_service::by_ide(&state.pool, &params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn top_agents(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = report_service::top_agents(&state.pool, &params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn error_patterns(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = report_service::error_patterns(&state.pool, &params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn cost_over_time(
    State(state): State<AppState>,
    Query(params): Query<CallsOverTimeParams>,
) -> Result<Json<Value>, AppError> {
    let q = ReportParams {
        from: params.from,
        to: params.to,
        repo: params.repo,
        ide: params.ide,
        agent: params.agent,
        model: params.model,
        skill: params.skill,
        limit: None,
    }
    .into_query();
    let bucket = params.bucket.unwrap_or_else(|| "day".into());
    let results = report_service::cost_over_time(&state.pool, &q, &bucket).await?;
    Ok(Json(json!(results)))
}

async fn distinct_models(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, AppError> {
    let models = report_service::distinct_models(&state.pool).await?;
    Ok(Json(models))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/reports", get(page))
        .route("/reports/top-tools", get(top_tools))
        .route("/reports/top-tasks", get(top_tasks))
        .route("/reports/top-mcp-servers", get(top_mcp_servers))
        .route("/reports/calls-over-time", get(calls_over_time))
        .route("/reports/by-ide", get(by_ide))
        .route("/reports/top-agents", get(top_agents))
        .route("/reports/error-patterns", get(error_patterns))
        .route("/reports/cost-over-time", get(cost_over_time))
        .route("/reports/models", get(distinct_models))
        .route("/reports/events", get(events_feed))
}

#[derive(Debug, Deserialize, Default)]
pub struct EventFeedParams {
    from: Option<String>,
    to: Option<String>,
    ide: Option<String>,
    agent: Option<String>,
    model: Option<String>,
    conversation_id: Option<String>,
    before_started_at: Option<String>,
    before_event_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

async fn events_feed(
    State(state): State<AppState>,
    Query(params): Query<EventFeedParams>,
) -> Result<Json<Value>, AppError> {
    let from = params.from.as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let to = params.to.as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let before_started_at = params.before_started_at.as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let before_event_id = params.before_event_id.as_deref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok());
    let q = EventQuery {
        from,
        to,
        ide: params.ide,
        agent: params.agent,
        model: params.model,
        conversation_id: params.conversation_id,
        before_started_at,
        before_event_id,
        limit: params.limit.unwrap_or(50).min(200),
        offset: params.offset.unwrap_or(0),
    };
    let results = report_service::events_feed(&state.pool, &q).await?;
    Ok(Json(json!(results)))
}
