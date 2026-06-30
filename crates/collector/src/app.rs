use axum::Router;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::middleware::rate_limit::RateLimiter;
use crate::routes;
use crate::services::ingest_buffer::IngestBuffer;
use agent_meter_db::Database;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Arc<dyn Database>,
    /// Async ingest buffer for fire-and-forget span writes.
    pub ingest: Option<IngestBuffer>,
    /// Per-IP rate limiter for ingest endpoints.
    pub rate_limiter: Arc<RateLimiter>,
}

pub fn build(config: Config, db: Arc<dyn Database>, cancel: CancellationToken) -> Router {
    let ingest = IngestBuffer::spawn(db.clone(), 4096, cancel);
    // 600 requests/min per IP (generous for batch telemetry)
    let rate_limiter = Arc::new(RateLimiter::new(600, 60));
    let state = AppState {
        config: Arc::new(config),
        db,
        ingest: Some(ingest),
        rate_limiter,
    };

    let router = Router::new()
        .merge(routes::dashboard::router())
        .merge(routes::health::router())
        .merge(routes::events::router())
        .merge(routes::reports::router())
        .merge(routes::conversations::router())
        .merge(routes::cost::router())
        .merge(routes::docs::router())
        .merge(routes::search::router())
        .merge(routes::static_assets::router())
        .fallback(routes::static_assets::not_found_page);

    router
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub fn build_otlp(config: Config, db: Arc<dyn Database>, cancel: CancellationToken) -> Router {
    let ingest = IngestBuffer::spawn(db.clone(), 4096, cancel);
    let rate_limiter = Arc::new(RateLimiter::new(600, 60));
    let state = AppState {
        config: Arc::new(config),
        db,
        ingest: Some(ingest),
        rate_limiter,
    };

    Router::new()
        .merge(routes::otlp::router())
        .with_state(state)
}
