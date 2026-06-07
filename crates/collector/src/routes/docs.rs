use axum::{response::Html, routing::get, Router};

const DOCS_HTML: &str = include_str!("../../ui/docs.html");

async fn handler() -> Html<&'static str> {
    Html(DOCS_HTML)
}

pub fn router() -> Router<crate::app::AppState> {
    Router::new().route("/docs", get(handler))
}
