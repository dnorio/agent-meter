mod app;
mod config;
mod db;
mod errors;
mod models;
mod routes;
mod services;
mod telemetry;

use std::net::SocketAddr;

use axum::serve;
use tokio::signal;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::from_env();

    telemetry::init_log(&config);
    let _otel_provider = telemetry::init_otel(&config);

    let pool = db::connect(&config.database_url).await?;

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
