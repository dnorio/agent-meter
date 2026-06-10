use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::app::AppState;
use crate::services::auth_service;

/// Extension inserted by the middleware when a valid API key is presented.
#[derive(Clone, Debug)]
pub struct AuthenticatedOrg {
    pub org_id: Uuid,
}

/// Axum middleware that enforces API key authentication.
/// Extracts `Authorization: Bearer am_live_...` and validates via auth_service.
pub async fn require_api_key(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    // Skip auth for paths that shouldn't require it
    let path = req.uri().path();
    if is_exempt_path(path) {
        return next.run(req).await;
    }

    // Only enforce on /api/* paths
    if !path.starts_with("/api/") {
        return next.run(req).await;
    }

    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let raw_key = if let Some(key) = auth_header.strip_prefix("Bearer ") {
        key.trim()
    } else {
        return unauthorized("missing Authorization: Bearer <api_key> header");
    };

    match auth_service::lookup_api_key(&state.pool, raw_key).await {
        Ok(Some(org_id)) => {
            req.extensions_mut().insert(AuthenticatedOrg { org_id });
            next.run(req).await
        }
        Ok(None) => unauthorized("invalid or revoked api key"),
        Err(_) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "auth service error"})),
            )
                .into_response()
        }
    }
}

fn is_exempt_path(path: &str) -> bool {
    matches!(
        path,
        "/health"
            | "/api/billing/webhook"
            | "/api/billing/plans"
            | "/api/stats/public"
            | "/api/status/db"
            | "/api/status/otlp"
            | "/api/status/pricing"
            | "/v1/traces"
    )
}

fn unauthorized(msg: &str) -> Response {
    (StatusCode::UNAUTHORIZED, Json(json!({"error": msg}))).into_response()
}
