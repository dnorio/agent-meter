use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;

use crate::app::AppState;
use crate::services::auth_service;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_page))
        .route("/auth/github", get(login_github))
        .route("/auth/github/callback", get(github_callback))
        .route("/auth/logout", post(logout).get(logout))
        .route("/api/me", get(me))
}

async fn login_page() -> impl IntoResponse {
    Html(include_str!("../../ui/login.html"))
}

#[derive(Deserialize)]
struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn login_github(State(state): State<AppState>) -> Response {
    let cid = match &state.config.github_client_id {
        Some(c) => c,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "GitHub OAuth not configured (set GITHUB_CLIENT_ID/SECRET)"})),
            )
                .into_response();
        }
    };
    let oauth_state = auth_service::make_oauth_state(&state.config.session_secret);
    let redirect = format!("{}/auth/github/callback", state.config.public_url);
    let url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&scope=read:user%20user:email&state={}",
        urlencode(cid),
        urlencode(&redirect),
        urlencode(&oauth_state)
    );
    Redirect::to(&url).into_response()
}

async fn github_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(p): Query<CallbackParams>,
) -> Response {
    if let Some(err) = p.error {
        return (StatusCode::BAD_REQUEST, format!("github error: {err}")).into_response();
    }
    let Some(code) = p.code else {
        return (StatusCode::BAD_REQUEST, "missing code").into_response();
    };
    let Some(st) = p.state else {
        return (StatusCode::BAD_REQUEST, "missing state").into_response();
    };
    if !auth_service::verify_oauth_state(&state.config.session_secret, &st) {
        return (StatusCode::BAD_REQUEST, "invalid state").into_response();
    }

    let (cid, csec) = match (&state.config.github_client_id, &state.config.github_client_secret) {
        (Some(a), Some(b)) => (a, b),
        _ => return (StatusCode::SERVICE_UNAVAILABLE, "oauth not configured").into_response(),
    };

    let redirect = format!("{}/auth/github/callback", state.config.public_url);
    let access_token = match auth_service::exchange_github_code(cid, csec, &code, &redirect).await {
        Ok(t) => t,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("github exchange: {e}")).into_response(),
    };
    let (gh, email) = match auth_service::fetch_github_user(&access_token).await {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("github user: {e}")).into_response(),
    };
    let user = match auth_service::upsert_github_user(&state.pool, &gh, &email).await {
        Ok(u) => u,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("upsert: {e}")).into_response(),
    };

    let ua = headers.get("user-agent").and_then(|v| v.to_str().ok());
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim());

    let token = match auth_service::create_session(&state.pool, user.user_id, user.org_id, ua, ip)
        .await
    {
        Ok(t) => t,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("session: {e}")).into_response(),
    };

    let cookie = format!(
        "am_session={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={}",
        token,
        60 * 60 * 24 * 30
    );

    let mut resp = Redirect::to("/").into_response();
    resp.headers_mut()
        .insert(header::SET_COOKIE, cookie.parse().unwrap());
    resp
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = extract_session_cookie(&headers) {
        let _ = auth_service::delete_session(&state.pool, &token).await;
    }
    let mut resp = Redirect::to("/login").into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        "am_session=; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=0"
            .parse()
            .unwrap(),
    );
    resp
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = extract_session_cookie(&headers) else {
        return (StatusCode::UNAUTHORIZED, Json(json!({"error": "not signed in"}))).into_response();
    };
    match auth_service::lookup_session(&state.pool, &token).await {
        Ok(Some(u)) => Json(json!({
            "user_id": u.user_id,
            "org_id": u.org_id,
            "email": u.email,
            "display_name": u.display_name,
            "avatar_url": u.avatar_url,
            "github_login": u.github_login,
        }))
        .into_response(),
        Ok(None) => (StatusCode::UNAUTHORIZED, Json(json!({"error": "session expired"})))
            .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
            .into_response(),
    }
}

pub fn extract_session_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("am_session=") {
            return Some(rest.to_string());
        }
    }
    None
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// Thin Html wrapper to avoid pulling axum::response::Html into upper modules.
struct Html<T>(pub T);
impl<T: Into<String>> IntoResponse for Html<T> {
    fn into_response(self) -> Response {
        let body: String = self.0.into();
        ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
    }
}
