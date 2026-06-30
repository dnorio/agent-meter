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
use tokio::signal;
use tokio_util::sync::CancellationToken;

pub async fn run(config: config::Config, db: Arc<dyn agent_meter_db::Database>) -> anyhow::Result<()> {
    let _otel_provider = telemetry::init_telemetry(&config);

    let token = CancellationToken::new();
    let token_clone = token.clone();
    let otlp_token = token.clone();

    let main_app = app::build(config.clone(), db.clone(), token.clone());
    let otlp_app = app::build_otlp(config.clone(), db.clone(), token.clone());

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    let otlp_addr: SocketAddr = format!("{}:{}", config.host, config.otlp_port).parse()?;

    tracing::info!(addr = %addr, "starting collector");
    tracing::info!(addr = %otlp_addr, "starting OTLP receiver");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let otlp_listener = tokio::net::TcpListener::bind(otlp_addr).await?;

    // Friendly standalone banner + open browser to the dashboard.
    let ui_url = format!("http://127.0.0.1:{}", config.port);
    eprintln!("\n  agent-meter is running");
    eprintln!("  ▸ Dashboard:     {ui_url}");
    eprintln!("  ▸ OTLP receiver: http://127.0.0.1:{}/v1/traces", config.otlp_port);
    eprintln!("  ▸ Press Ctrl+C to stop\n");
    if std::env::var("AGENT_METER_NO_OPEN").is_err() {
        let url = ui_url.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
            open_browser(&url);
        });
    }

    tokio::spawn(async move {
        signal::ctrl_c().await.ok();
        tracing::info!("shutdown signal received");
        token_clone.cancel();
    });

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

/// Best-effort open of the default browser (Linux/macOS/Windows). Errors ignored.
fn open_browser(url: &str) {
    #[cfg(target_os = "linux")]
    let cmd = ("xdg-open", vec![url]);
    #[cfg(target_os = "macos")]
    let cmd = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let cmd = ("cmd", vec!["/C", "start", "", url]);

    let _ = std::process::Command::new(cmd.0)
        .args(cmd.1)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
