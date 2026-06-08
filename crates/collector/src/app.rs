use axum::Router;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::middleware::api_key_auth;
use crate::routes;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: PgPool,
}

pub fn build(config: Config, pool: PgPool) -> Router {
    let require_api_key = config.require_api_key;
    let state = AppState {
        config: Arc::new(config),
        pool,
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
        .merge(routes::billing::router())
        .merge(routes::budgets::router())
        .merge(routes::notifications::router())
        .merge(routes::leaderboard::router())
        .merge(routes::docs::router())
        .merge(routes::search::router())
        .merge(routes::static_assets::router())
        .fallback(routes::static_assets::not_found_page);

    if require_api_key {
        router = router.layer(axum::middleware::from_fn_with_state(
            state.clone(),
            api_key_auth::require_api_key,
        ));
    }

    router
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub fn build_otlp(config: Config, pool: PgPool) -> Router {
    let state = AppState {
        config: Arc::new(config),
        pool,
    };

    Router::new()
        .merge(routes::otlp::router())
        .with_state(state)
}
