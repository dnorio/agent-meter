//! T-322 — Status page, ToS, Privacy routes + status check endpoints

use axum::{
    response::Html,
    routing::get,
    Router,
};

use crate::app::AppState;

const STATUS_HTML: &str = include_str!("../../ui/status.html");
const TERMS_HTML: &str = include_str!("../../ui/terms.html");
const PRIVACY_HTML: &str = include_str!("../../ui/privacy.html");

// --- Pages ---

async fn status_page() -> Html<&'static str> {
    Html(STATUS_HTML)
}
async fn terms_page() -> Html<&'static str> {
    Html(TERMS_HTML)
}
async fn privacy_page() -> Html<&'static str> {
    Html(PRIVACY_HTML)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/status", get(status_page))
        .route("/terms", get(terms_page))
        .route("/privacy", get(privacy_page))
}
