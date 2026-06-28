use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use agent_meter_db::Database;
use crate::config::Config;
use crate::middleware::api_key_auth;
use crate::middleware::rate_limit::RateLimiter;
use crate::routes;
use crate::services::ingest_buffer::IngestBuffer;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Arc<dyn Database>,
    /// Escape hatch: direct pool access during migration period.
    /// Services should migrate to use `db` trait methods.
    pub pool: PgPool,
    /// Async ingest buffer for fire-and-forget span writes.
    pub ingest: Option<IngestBuffer>,
    /// Per-IP rate limiter for ingest endpoints.
    pub rate_limiter: Arc<RateLimiter>,
}

pub fn build(config: Config, pool: PgPool, db: Arc<dyn Database>, cancel: CancellationToken) -> Router {
    let require_api_key = config.require_api_key;
    let ingest = IngestBuffer::spawn(pool.clone(), 4096, cancel);
    // 600 requests/min per IP (generous for batch telemetry)
    let rate_limiter = Arc::new(RateLimiter::new(600, 60));
    let state = AppState {
        config: Arc::new(config),
        db,
        pool,
        ingest: Some(ingest),
        rate_limiter,
    };

    let mut router = Router::new()
        .merge(routes::dashboard::router())
        .merge(routes::health::router())
        .merge(routes::events::router())
        .merge(routes::reports::router())
        .merge(routes::tasks::router())
        .merge(routes::conversations::router())
        .merge(routes::conversation_detail::router())
        .merge(routes::cost::router())
        .merge(routes::export::router())
        .merge(routes::orgs::router())
        .merge(routes::alerts::router())
        .merge(routes::auth::router())
        .merge(routes::badge::router())
        .merge(routes::billing::router())
        .merge(routes::budgets::router())
        .merge(routes::notifications::router())
        .merge(routes::leaderboard::router())
        .merge(routes::hosted::router())
        .merge(routes::docs::router())
        .merge(routes::search::router())
        .merge(routes::setup::router())
        .merge(routes::static_assets::router())
        .fallback(routes::static_assets::not_found_page);

    if require_api_key {
        router = router.layer(axum::middleware::from_fn_with_state(
            state.clone(),
            api_key_auth::require_api_key,
        ));
    }

    router
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub fn build_otlp(config: Config, pool: PgPool, db: Arc<dyn Database>, cancel: CancellationToken) -> Router {
    let ingest = IngestBuffer::spawn(pool.clone(), 4096, cancel);
    let rate_limiter = Arc::new(RateLimiter::new(600, 60));
    let state = AppState {
        config: Arc::new(config),
        db,
        pool,
        ingest: Some(ingest),
        rate_limiter,
    };

    Router::new()
        .merge(routes::otlp::router())
        .with_state(state)
}
