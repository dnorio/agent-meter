use axum::{response::Html, routing::get, Router};

const DASHBOARD_HTML: &str = include_str!("../../ui/dashboard.html");

async fn handler() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

pub fn router() -> Router<crate::app::AppState> {
    Router::new().route("/", get(handler))
}
