//! T-319 — Rotas Organizations & API Keys.

use axum::{
    extract::{Path, State},
    response::Html,
    routing::{delete, get},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::AppError;
use crate::services::org_service;

const SETTINGS_HTML: &str = include_str!("../../ui/settings.html");

async fn settings_page() -> Html<&'static str> {
    Html(SETTINGS_HTML)
}

async fn list_orgs_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<org_service::Organization>>, AppError> {
    let orgs = org_service::list_orgs(&state.pool).await?;
    Ok(Json(orgs))
}

async fn list_keys_handler(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
) -> Result<Json<Vec<org_service::ApiKey>>, AppError> {
    let keys = org_service::list_api_keys(&state.pool, org_id).await?;
    Ok(Json(keys))
}

#[derive(Debug, Deserialize)]
pub struct CreateKeyBody {
    pub name: Option<String>,
}

async fn create_key_handler(
    State(state): State<AppState>,
    Path(org_id): Path<Uuid>,
    Json(body): Json<CreateKeyBody>,
) -> Result<Json<org_service::CreatedApiKey>, AppError> {
    let name = body.name.unwrap_or_else(|| "default".to_string());
    let created = org_service::create_api_key(&state.pool, org_id, &name).await?;
    Ok(Json(created))
}

async fn revoke_key_handler(
    State(state): State<AppState>,
    Path((_org_id, key_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    org_service::revoke_api_key(&state.pool, key_id).await?;
    Ok(Json(serde_json::json!({"ok": true})))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings", get(settings_page))
        .route("/api/orgs", get(list_orgs_handler))
        .route("/api/orgs/:org_id/keys", get(list_keys_handler).post(create_key_handler))
        .route("/api/orgs/:org_id/keys/:key_id", delete(revoke_key_handler))
}
