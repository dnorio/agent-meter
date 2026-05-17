use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::State,
    http::HeaderMap,
    routing::post,
    Json, Router,
};
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone)]
struct AppConfig {
    upstream_url: String,
    collector_url: String,
    listen_addr: SocketAddr,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_target(true)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = AppConfig {
        upstream_url: env::var("MCP_UPSTREAM_URL")
            .unwrap_or_else(|_| "http://localhost:3001".into()),
        collector_url: env::var("AGENT_METER_COLLECTOR_URL")
            .unwrap_or_else(|_| "http://localhost:8081".into()),
        listen_addr: env::var("MCP_WRAPPER_LISTEN")
            .unwrap_or_else(|_| "0.0.0.0:3001".into())
            .parse()
            .expect("invalid MCP_WRAPPER_LISTEN"),
    };

    tracing::info!(
        upstream = %config.upstream_url,
        collector = %config.collector_url,
        listen = %config.listen_addr,
        "starting MCP wrapper"
    );

    let listen_addr = config.listen_addr;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let state = Arc::new((config, client));

    let app = Router::new()
        .route("/", post(proxy_handler))
        .route("/health", axum::routing::get(|| async { Json(serde_json::json!({"status":"ok"})) }))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(listen_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn proxy_handler(
    State(state): State<Arc<(AppConfig, reqwest::Client)>>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, axum::response::Json<Value>> {
    let (config, client) = state.as_ref();

    let request_bytes = body.len() as i32;

    let request_body: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return Err(Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": format!("Parse error: {e}") }
            })));
        }
    };

    let method = request_body["method"].as_str().unwrap_or("");
    let is_mcp_method = matches!(method, "tools/list" | "tools/call" | "tools/notify" | "initialize" | "notifications/initialized");

    let started_at = Utc::now();

    let mut proxy_headers = HeaderMap::new();
    for (key, value) in headers.iter() {
        if key.as_str().starts_with("x-") || *key == "content-type" || *key == "accept" {
            proxy_headers.insert(key.clone(), value.clone());
        }
    }

    let proxy_resp = match client
        .post(&config.upstream_url)
        .headers(proxy_headers)
        .header("content-type", "application/json")
        .body(body.clone())
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err(Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request_body.get("id"),
                "error": { "code": -32603, "message": format!("upstream error: {e}") }
            })));
        }
    };

    let ended_at = Utc::now();
    let duration_ms = (ended_at - started_at).num_milliseconds() as i32;
    let status = proxy_resp.status();
    let ok = status.is_success();

    let proxy_body = proxy_resp.text().await.unwrap_or_default();
    let response_bytes = proxy_body.len() as i32;

    let response_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(proxy_body.as_bytes());
        hex::encode(hasher.finalize())
    };
    let request_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(body.as_bytes());
        hex::encode(hasher.finalize())
    };

    let upstream_body: Value = serde_json::from_str(&proxy_body).unwrap_or(Value::Null);
    let is_error = upstream_body.get("error").is_some()
        || upstream_body.get("result")
            .and_then(|r| r.get("isError"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

    if is_mcp_method {
        let tool_name = if method == "tools/call" {
            request_body["params"]["name"].as_str().unwrap_or("unknown")
        } else {
            method
        };

        let event_id = Uuid::new_v4();

        let event = serde_json::json!({
            "event_id": event_id.to_string(),
            "task_id": env::var("AGENT_METER_TASK_ID").ok(),
            "repo": env::var("AGENT_METER_REPO").ok(),
            "branch": env::var("AGENT_METER_BRANCH").ok(),
            "ide": env::var("AGENT_METER_IDE").ok(),
            "agent": env::var("AGENT_METER_AGENT").ok(),
            "skill": env::var("AGENT_METER_SKILL").ok(),
            "mcp_server": env::var("MCP_SERVER_NAME").unwrap_or_else(|_| "upstream".into()),
            "tool_name": tool_name,
            "started_at": started_at.to_rfc3339(),
            "ended_at": ended_at.to_rfc3339(),
            "ok": ok && !is_error,
            "error": upstream_body.get("error").map(|e| e.to_string()),
            "request_bytes": request_bytes,
            "response_bytes": response_bytes,
            "request_sha256": request_sha256,
            "response_sha256": response_sha256,
            "metadata": {
                "method": method,
                "http_status": status.as_u16(),
                "duration_ms": duration_ms,
                "wrapper_upstream": config.upstream_url,
            }
        });

        let collector_url = config.collector_url.clone();
        let event_clone = event.clone();

        tokio::spawn(async move {
            let c = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build().ok();

            if let Some(client) = c {
                if let Err(e) = client
                    .post(format!("{}/events/tool-call", collector_url))
                    .json(&event_clone)
                    .send()
                    .await
                {
                    tracing::warn!(error = %e, "failed to send event to collector");
                }
            }
        });
    }

    let proxy_response: Value = serde_json::from_str(&proxy_body).unwrap_or_else(|_| {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_body.get("id"),
            "result": proxy_body,
        })
    });

    Ok(Json(proxy_response))
}
