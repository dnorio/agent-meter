use axum::{response::{Html, Redirect}, routing::get, Router};

const DASHBOARD_HTML: &str = include_str!("../../ui/dashboard.html");

async fn handler() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

async fn redirect_to_root() -> Redirect {
    Redirect::permanent("/")
}

pub fn router() -> Router<crate::app::AppState> {
    Router::new()
        .route("/", get(handler))
        .route("/dashboard", get(redirect_to_root))
}
