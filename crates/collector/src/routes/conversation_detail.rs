use axum::{extract::Path, response::Html, routing::get, Router};

const TIMELINE_HTML: &str = include_str!("../../ui/timeline.html");

async fn handler(Path(_conversation_id): Path<String>) -> Html<&'static str> {
    Html(TIMELINE_HTML)
}

pub fn router() -> Router<crate::app::AppState> {
    Router::new().route("/conversations/:conversation_id/timeline", get(handler))
}