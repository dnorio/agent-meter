//! Report routes — uses Database trait directly (no service passthrough).

use axum::{
    extract::{Query, State},
    response::Html,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use agent_meter_db::params::{EventQuery, ReportQuery};

use crate::app::AppState;
use crate::errors::AppError;

const REPORTS_HTML: &str = include_str!("../../ui/reports.html");

async fn page() -> Html<&'static str> {
    Html(REPORTS_HTML)
}

// ── Query params ────────────────────────────────────────────────────────────

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
        ReportQuery {
            from: parse_dt(&self.from),
            to: parse_dt(&self.to),
            repo: self.repo,
            ide: self.ide,
            agent: self.agent,
            model: self.model,
            skill: self.skill,
            limit: self.limit,
        }
    }
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

// ── Handlers ────────────────────────────────────────────────────────────────

async fn top_tools(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = state.db.top_tools(&params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn top_tasks(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = state.db.top_tasks(&params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn top_mcp_servers(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = state.db.top_mcp_servers(&params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn calls_over_time(
    State(state): State<AppState>,
    Query(params): Query<CallsOverTimeParams>,
) -> Result<Json<Value>, AppError> {
    let q = ReportQuery {
        from: parse_dt(&params.from),
        to: parse_dt(&params.to),
        repo: params.repo,
        ide: params.ide,
        agent: params.agent,
        model: params.model,
        skill: params.skill,
        limit: None,
    };
    let bucket = params.bucket.unwrap_or_else(|| "hour".into());
    let results = state.db.calls_over_time(&q, &bucket).await?;
    Ok(Json(json!(results)))
}

async fn by_ide(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = state.db.ide_breakdown(&params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn top_agents(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = state.db.top_agents(&params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn error_patterns(
    State(state): State<AppState>,
    Query(params): Query<ReportParams>,
) -> Result<Json<Value>, AppError> {
    let results = state.db.error_patterns(&params.into_query()).await?;
    Ok(Json(json!(results)))
}

async fn cost_over_time(
    State(state): State<AppState>,
    Query(params): Query<CallsOverTimeParams>,
) -> Result<Json<Value>, AppError> {
    let q = ReportQuery {
        from: parse_dt(&params.from),
        to: parse_dt(&params.to),
        repo: params.repo,
        ide: params.ide,
        agent: params.agent,
        model: params.model,
        skill: params.skill,
        limit: None,
    };
    let _bucket = params.bucket.unwrap_or_else(|| "day".into());
    // cost_over_time uses the trait which groups by hour; bucket param is for future use
    let results = state.db.cost_over_time(&q).await?;
    Ok(Json(json!(results)))
}

async fn distinct_models(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, AppError> {
    let models = state.db.distinct_models().await?;
    Ok(Json(models))
}

async fn events_feed(
    State(state): State<AppState>,
    Query(params): Query<EventFeedParams>,
) -> Result<Json<Value>, AppError> {
    let q = EventQuery {
        from: parse_dt(&params.from),
        to: parse_dt(&params.to),
        ide: params.ide,
        agent: params.agent,
        model: params.model,
        conversation_id: params.conversation_id,
        before_started_at: parse_dt(&params.before_started_at),
        before_event_id: params.before_event_id.as_deref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok()),
        limit: params.limit.unwrap_or(50).min(200),
        offset: params.offset.unwrap_or(0),
    };
    let results = state.db.query_events(&q).await?;
    Ok(Json(json!(results)))
}

// ── Router ──────────────────────────────────────────────────────────────────

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

// ── Helpers ─────────────────────────────────────────────────────────────────

fn parse_dt(s: &Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
    s.as_deref()
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
}
