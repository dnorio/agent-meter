use axum::{extract::{Query, State}, response::Html, routing::get, routing::post, Json, Router};
use serde::Deserialize;
use serde_json::Value;

use crate::app::AppState;
use crate::errors::AppError;
use crate::services::task_service;

const TASKS_HTML: &str = include_str!("../../ui/tasks.html");

async fn page() -> Html<&'static str> {
    Html(TASKS_HTML)
}

#[derive(Debug, Deserialize)]
pub struct StartTaskRequest {
    pub task_id: String,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub ide: Option<String>,
    pub agent: Option<String>,
    pub skill: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EndTaskRequest {
    pub task_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ListTasksParams {
    limit: Option<i64>,
}

async fn post_start(
    State(state): State<AppState>,
    Json(req): Json<StartTaskRequest>,
) -> Result<Json<Value>, AppError> {
    let result = task_service::start_task(
        &state.pool,
        &req.task_id,
        req.repo.as_deref(),
        req.branch.as_deref(),
        req.ide.as_deref(),
        req.agent.as_deref(),
        req.skill.as_deref(),
    )
    .await?;
    Ok(Json(result))
}

async fn post_end(
    State(state): State<AppState>,
    Json(req): Json<EndTaskRequest>,
) -> Result<Json<Value>, AppError> {
    let result = task_service::end_task(&state.pool, &req.task_id).await?;
    Ok(Json(result))
}

async fn get_tasks(
    State(state): State<AppState>,
    Query(params): Query<ListTasksParams>,
) -> Result<Json<Value>, AppError> {
    let result = task_service::list_tasks(&state.pool, params.limit.unwrap_or(20)).await?;
    Ok(Json(Value::Array(result)))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tasks/start", post(post_start))
        .route("/tasks/end", post(post_end))
        .route("/api/tasks", get(get_tasks))
        .route("/tasks", get(page))
}
