pub mod app;
pub mod config;
pub mod db;
pub mod errors;
pub mod middleware;
pub mod models;
pub mod otlp;
pub mod routes;
pub mod services;
pub mod telemetry;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::serve;
use sqlx::PgPool;
use tokio::signal;
use tokio_util::sync::CancellationToken;

use agent_meter_db::PostgresDb;

pub async fn run(config: config::Config, pool: PgPool) -> anyhow::Result<()> {
    let _otel_provider = telemetry::init_telemetry(&config);

    let db: Arc<dyn agent_meter_db::Database> = Arc::new(PostgresDb::from_pool(pool.clone()));

    let token = CancellationToken::new();
    let token_clone = token.clone();
    let otlp_token = token.clone();
    let pricing_token = token.clone();

    let main_app = app::build(config.clone(), pool.clone(), db.clone(), token.clone());
    let otlp_app = app::build_otlp(config.clone(), pool.clone(), db.clone(), token.clone());

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let otlp_addr: SocketAddr = format!("{}:{}", config.host, config.otlp_port).parse()?;

    tracing::info!(addr = %addr, "starting collector");
    tracing::info!(addr = %otlp_addr, "starting OTLP receiver");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let otlp_listener = tokio::net::TcpListener::bind(otlp_addr).await?;

    tokio::spawn(async move {
        signal::ctrl_c().await.ok();
        tracing::info!("shutdown signal received");
        token_clone.cancel();
    });

    // T-360: Background pricing auto-sync (every 24h)
    services::pricing_updater::spawn_pricing_updater(pool.clone(), pricing_token);

    let main_handle = tokio::spawn(async move {
        if let Err(e) = serve(listener, main_app)
            .with_graceful_shutdown(async move { token.cancelled().await })
            .await
        {
            tracing::error!(error = %e, "main server failed");
        }
    });

    let otlp_handle = tokio::spawn(async move {
        if let Err(e) = serve(otlp_listener, otlp_app.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(async move { otlp_token.cancelled().await })
            .await
        {
            tracing::error!(error = %e, "OTLP server failed");
        }
    });

    let _ = tokio::join!(main_handle, otlp_handle);

    if let Some(provider) = _otel_provider {
        if let Err(e) = provider.shutdown() {
            tracing::error!(error = %e, "OTEL shutdown failed");
        }
    }

    tracing::info!("collector stopped");
    Ok(())
}
