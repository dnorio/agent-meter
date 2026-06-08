use std::{env, sync::Arc};

use axum::{
    extract::State,
    http::HeaderMap,
    routing::post,
    Json, Router,
};
use chrono::Utc;
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone)]
pub struct ProxyState {
    pub upstream_url: String,
    pub collector_url: String,
    pub client: Client,
}

pub fn router(upstream_url: String, collector_url: String, client: Client) -> Router {
    let state = Arc::new(ProxyState {
        upstream_url,
        collector_url,
        client,
    });

    Router::new()
        .route("/", post(proxy_handler))
        .route("/health", axum::routing::get(|| async { Json(serde_json::json!({"status":"ok"})) }))
        .with_state(state)
}

async fn proxy_handler(
    state: State<Arc<ProxyState>>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, Json<Value>> {
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
    let is_mcp_method =
        matches!(method, "tools/list" | "tools/call" | "tools/notify" | "initialize" | "notifications/initialized");

    let started_at = Utc::now();

    let mut proxy_headers = HeaderMap::new();
    for (key, value) in headers.iter() {
        if key.as_str().starts_with("x-") || *key == "content-type" || *key == "accept" {
            proxy_headers.insert(key.clone(), value.clone());
        }
    }

    let proxy_resp = match state
        .client
        .post(&state.upstream_url)
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
        || upstream_body
            .get("result")
            .and_then(|r| r.get("isError"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

    if is_mcp_method {
        let tool_name = if method == "tools/call" {
            request_body["params"]["name"].as_str().unwrap_or("unknown")
        } else {
            method
        };

        // Capture full tool arguments (input) for tools/call
        let tool_arguments: Option<Value> = if method == "tools/call" {
            request_body["params"].get("arguments").cloned()
        } else {
            None
        };

        // Capture tool result content (output), truncated at 8 KB
        let tool_result: Option<String> = if method == "tools/call" {
            extract_tool_result(&upstream_body)
        } else {
            None
        };

        let event_id = Uuid::new_v4();

        // JSON-RPC id — used as tool_call_id for correlation with LLM responses
        let tool_call_id = request_body.get("id").map(|v| v.to_string());

        // T-340: IDE identification — env var takes priority, then X-Agent-IDE header
        let ide_value = env::var("AGENT_METER_IDE").ok().or_else(|| {
            headers
                .get("x-agent-ide")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        });

        let event = serde_json::json!({
            "event_id": event_id.to_string(),
            "task_id": env::var("AGENT_METER_TASK_ID").ok(),
            "repo": env::var("AGENT_METER_REPO").ok(),
            "branch": env::var("AGENT_METER_BRANCH").ok(),
            "ide": ide_value,
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
            "tool_arguments": tool_arguments,
            "tool_result": tool_result,
            "tool_call_id": tool_call_id,
            "metadata": {
                "method": method,
                "http_status": status.as_u16(),
                "wrapper_upstream": state.upstream_url,
            }
        });

        let collector_url = state.collector_url.clone();
        let event_clone = event.clone();

        tokio::spawn(async move {
            let c = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .ok();

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

/// Extract human-readable content from a tools/call MCP response.
/// MCP result format: { "result": { "content": [{"type":"text","text":"..."},...], "isError": false } }
/// Concatenates all text blocks, truncated to 8 KB.
fn extract_tool_result(body: &Value) -> Option<String> {
    const MAX_BYTES: usize = 8 * 1024;

    let content = body.pointer("/result/content")?;
    let blocks = content.as_array()?;

    let mut parts: Vec<String> = Vec::new();
    for block in blocks {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        let text = match block_type {
            "text" => block.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string(),
            "image" => "[image]".to_string(),
            "resource" => block
                .get("resource")
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "[resource]".to_string()),
            _ => block.to_string(),
        };
        if !text.is_empty() {
            parts.push(text);
        }
    }

    if parts.is_empty() {
        // Fallback: if result has no content array, stringify result directly
        let raw = body.pointer("/result").map(|v| v.to_string()).unwrap_or_default();
        if raw.is_empty() || raw == "null" { return None; }
        let truncated = truncate_utf8(&raw, MAX_BYTES);
        return Some(truncated);
    }

    let joined = parts.join("\n");
    Some(truncate_utf8(&joined, MAX_BYTES))
}

fn truncate_utf8(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    // Find a valid UTF-8 boundary
    let mut end = max_bytes;
    while !s.is_char_boundary(end) { end -= 1; }
    format!("{}…[truncated]", &s[..end])
}
