pub mod app;
pub mod config;
pub mod db;
pub mod errors;
pub mod models;
pub mod routes;
pub mod services;
pub mod telemetry;

use std::net::SocketAddr;

use axum::serve;
use sqlx::PgPool;
use tokio::signal;
use tokio_util::sync::CancellationToken;

pub async fn run(config: config::Config, pool: PgPool) -> anyhow::Result<()> {
    let _otel_provider = telemetry::init_telemetry(&config);

    let app = app::build(config.clone(), pool);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!(addr = %addr, "starting collector");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let token = CancellationToken::new();
    let token_clone = token.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.ok();
        tracing::info!("shutdown signal received");
        token_clone.cancel();
    });

    serve(listener, app)
        .with_graceful_shutdown(async move { token.cancelled().await })
        .await?;

    if let Some(provider) = _otel_provider {
        if let Err(e) = provider.shutdown() {
            tracing::error!(error = %e, "OTEL shutdown failed");
        }
    }

    tracing::info!("collector stopped");
    Ok(())
}
