use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::app::AppState;
use crate::services::stripe_service;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/pricing", get(pricing_page))
        .route("/billing/stub", get(stub_page))
        .route("/api/billing/checkout", post(checkout))
        .route("/api/billing/portal", post(portal))
        .route("/api/billing/webhook", post(webhook))
}

async fn pricing_page() -> impl IntoResponse {
    HtmlResp(include_str!("../../ui/pricing.html"))
}

async fn stub_page() -> impl IntoResponse {
    HtmlResp(
        r#"<!doctype html><html><body style="background:#0a0a0f;color:#e4e4ed;font-family:sans-serif;padding:40px;text-align:center">
        <h1>Stripe stub</h1>
        <p>This is a development placeholder. Configure <code>STRIPE_SECRET_KEY</code> for real checkout.</p>
        <p><a href="/" style="color:#6366f1">← back</a></p>
        </body></html>"#,
    )
}

#[derive(Deserialize)]
struct CheckoutBody {
    plan: String, // "pro" | "team"
    org_id: Option<Uuid>,
}

async fn checkout(State(state): State<AppState>, Json(body): Json<CheckoutBody>) -> Response {
    let price_id = match body.plan.as_str() {
        "pro" => state.config.stripe_price_pro.clone(),
        "team" => state.config.stripe_price_team.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "unknown plan"})),
            )
                .into_response();
        }
    };
    let price_id = price_id.unwrap_or_else(|| format!("price_stub_{}", body.plan));

    // pick org: from body or fallback to "personal"
    let org_id = match body.org_id {
        Some(id) => id,
        None => {
            match sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM organizations WHERE slug = 'personal' LIMIT 1",
            )
            .fetch_optional(&state.pool)
            .await
            {
                Ok(Some(id)) => id,
                _ => {
                    return (StatusCode::BAD_REQUEST, Json(json!({"error": "no org"})))
                        .into_response()
                }
            }
        }
    };

    let success = format!("{}/?billing=success", state.config.public_url);
    let cancel = format!("{}/pricing?billing=cancel", state.config.public_url);
    match stripe_service::create_checkout(
        state.config.stripe_secret_key.as_deref(),
        &price_id,
        org_id,
        &success,
        &cancel,
    )
    .await
    {
        Ok(r) => Json(json!({"url": r.url, "mode": r.mode})).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct PortalBody {
    org_id: Uuid,
}

async fn portal(State(state): State<AppState>, Json(body): Json<PortalBody>) -> Response {
    let cust: Option<String> =
        match sqlx::query_scalar("SELECT stripe_customer_id FROM organizations WHERE id = $1")
            .bind(body.org_id)
            .fetch_optional(&state.pool)
            .await
        {
            Ok(v) => v.unwrap_or(None),
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response();
            }
        };
    let Some(customer_id) = cust else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "org has no stripe customer"})),
        )
            .into_response();
    };
    let return_url = format!("{}/", state.config.public_url);
    match stripe_service::create_portal(
        state.config.stripe_secret_key.as_deref(),
        &customer_id,
        &return_url,
    )
    .await
    {
        Ok(url) => Json(json!({"url": url})).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn webhook(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(secret) = state.config.stripe_webhook_secret.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "STRIPE_WEBHOOK_SECRET not configured"})),
        )
            .into_response();
    };
    let sig = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !stripe_service::verify_webhook_signature(&secret, &body, sig) {
        return (StatusCode::BAD_REQUEST, "invalid signature").into_response();
    }
    let ev: stripe_service::StripeEvent = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("invalid payload: {e}")).into_response();
        }
    };
    if let Err(e) = stripe_service::record_event(&state.pool, &ev).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("record: {e}"),
        )
            .into_response();
    }
    Json(json!({"received": true, "type": ev.event_type})).into_response()
}

// Helper to return raw HTML.
struct HtmlResp(&'static str);
impl IntoResponse for HtmlResp {
    fn into_response(self) -> Response {
        ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], self.0).into_response()
    }
}
