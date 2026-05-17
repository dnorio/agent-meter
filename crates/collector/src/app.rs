use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::routes;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: PgPool,
}

pub fn build(config: Config, pool: PgPool) -> Router {
    let state = AppState {
        config: Arc::new(config),
        pool,
    };

    Router::new()
        .merge(routes::dashboard::router())
        .merge(routes::health::router())
        .merge(routes::events::router())
        .merge(routes::reports::router())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
