use std::env;

use agent_meter_mcp_wrapper::proxy;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_target(true)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let listen_addr: std::net::SocketAddr = env::var("MCP_WRAPPER_LISTEN")
        .unwrap_or_else(|_| "0.0.0.0:3001".into())
        .parse()
        .expect("invalid MCP_WRAPPER_LISTEN");

    let upstream_url = env::var("MCP_UPSTREAM_URL")
        .unwrap_or_else(|_| "http://localhost:3001".into());
    let collector_url = env::var("AGENT_METER_COLLECTOR_URL")
        .unwrap_or_else(|_| "http://localhost:8081".into());

    tracing::info!(
        upstream = %upstream_url,
        collector = %collector_url,
        listen = %listen_addr,
        "starting MCP wrapper"
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let app = proxy::router(upstream_url, collector_url, client);

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
