//! T-352 — Notification Channels routes (CRUD + test dispatch)

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::AppError;
use crate::services::notification_service::{
    self, CreateChannel, DispatchResult, NotificationChannel, UpdateChannel,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/notifications/channels",
            get(list_channels).post(create_channel),
        )
        .route(
            "/api/notifications/channels/{id}",
            get(get_channel).put(update_channel).delete(delete_channel),
        )
        .route("/api/notifications/test", post(test_dispatch))
}

async fn list_channels(
    State(state): State<AppState>,
) -> Result<Json<Vec<NotificationChannel>>, AppError> {
    let channels = notification_service::list(&state.pool).await?;
    Ok(Json(channels))
}

async fn get_channel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<NotificationChannel>, AppError> {
    let ch = notification_service::get(&state.pool, id).await?;
    Ok(Json(ch))
}

async fn create_channel(
    State(state): State<AppState>,
    Json(input): Json<CreateChannel>,
) -> Result<Json<NotificationChannel>, AppError> {
    let ch = notification_service::create(&state.pool, input).await?;
    Ok(Json(ch))
}

async fn update_channel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateChannel>,
) -> Result<Json<NotificationChannel>, AppError> {
    let ch = notification_service::update(&state.pool, id, input).await?;
    Ok(Json(ch))
}

async fn delete_channel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    notification_service::delete(&state.pool, id).await?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

#[derive(Deserialize)]
struct TestPayload {
    subject: Option<String>,
    body: Option<String>,
}

async fn test_dispatch(
    State(state): State<AppState>,
    Json(input): Json<TestPayload>,
) -> Result<Json<Vec<DispatchResult>>, AppError> {
    let subject = input.subject.as_deref().unwrap_or("Test notification");
    let body = input
        .body
        .as_deref()
        .unwrap_or("This is a test from agent-meter.");
    let results = notification_service::dispatch(&state.pool, subject, body).await?;
    Ok(Json(results))
}
