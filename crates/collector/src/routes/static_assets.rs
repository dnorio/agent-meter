use axum::{
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

const TOKENS_CSS: &str = include_str!("../../ui/_static/tokens.css");
const APP_CSS: &str = include_str!("../../ui/_static/app.css");
const APP_JS: &str = include_str!("../../ui/_static/app.js");
const ICONS_SVG: &str = include_str!("../../ui/_static/icons.svg");
const FAVICON_SVG: &str = include_str!("../../ui/_static/favicon.svg");

fn css(body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8"),
      (header::CACHE_CONTROL, "public, max-age=300")], body).into_response()
}
fn js(body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, "application/javascript; charset=utf-8"),
      (header::CACHE_CONTROL, "public, max-age=300")], body).into_response()
}
fn svg(body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"),
      (header::CACHE_CONTROL, "public, max-age=3600")], body).into_response()
}

pub fn router() -> Router<crate::app::AppState> {
    Router::new()
        .route("/_static/tokens.css", get(|| async { css(TOKENS_CSS) }))
        .route("/_static/app.css", get(|| async { css(APP_CSS) }))
        .route("/_static/app.js", get(|| async { js(APP_JS) }))
        .route("/_static/icons.svg", get(|| async { svg(ICONS_SVG) }))
        .route("/_static/favicon.svg", get(|| async { svg(FAVICON_SVG) }))
        .route("/favicon.svg", get(|| async { svg(FAVICON_SVG) }))
        .route("/favicon.ico", get(|| async { svg(FAVICON_SVG) }))
}
