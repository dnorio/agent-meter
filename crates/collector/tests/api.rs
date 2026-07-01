use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_meter_collector::{app, config::Config};
use agent_meter_db::{Database, SqliteDb};
use reqwest::Client;
use serde_json::json;
use tokio_util::sync::CancellationToken;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Fresh, isolated SQLite database per test (temp file).
async fn make_db() -> Arc<dyn Database> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("am-test-api-{nanos}-{n}.db"));
    let url = format!("sqlite://{}", path.display());
    let db = SqliteDb::connect(&url)
        .await
        .unwrap_or_else(|e| panic!("sqlite connect: {e}"));
    db.migrate()
        .await
        .unwrap_or_else(|e| panic!("sqlite migrate: {e}"));
    Arc::new(db)
}

async fn setup() -> (String, Client) {
    let db = make_db().await;
    let config = Config::from_env();
    let app = app::build(config, db, CancellationToken::new());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    (base_url, Client::new())
}

#[tokio::test]
async fn test_health() {
    let (base_url, client) = setup().await;
    let resp = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "agent-meter-collector");
}

#[tokio::test]
async fn test_dashboard_html() {
    let (base_url, client) = setup().await;
    let resp = client.get(format!("{}/", base_url)).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("<!DOCTYPE html>"));
    assert!(body.contains("agent-meter"));
}

#[tokio::test]
async fn test_post_tool_call_event() {
    let (base_url, client) = setup().await;
    let event = json!({
        "event_id": uuid::Uuid::new_v4().to_string(),
        "tool_name": "integration_test_tool",
        "mcp_server": "filesystem",
        "conversation_id": "conv-test-1",
        "started_at": "2026-05-17T00:00:00Z",
        "ended_at": "2026-05-17T00:00:01Z",
        "ok": true,
        "request_bytes": 100,
        "response_bytes": 500
    });
    let resp = client
        .post(format!("{}/events/tool-call", base_url))
        .json(&event)
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let text = resp.text().await.unwrap();
    assert_eq!(status, 200, "event should be accepted: {}", text);
}

#[tokio::test]
async fn test_reports_top_tools() {
    let (base_url, client) = setup().await;
    let resp = client
        .get(format!("{}/reports/top-tools", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_array(), "top-tools should be an array");
}

#[tokio::test]
async fn test_reports_top_mcp_servers() {
    let (base_url, client) = setup().await;
    let resp = client
        .get(format!("{}/reports/top-mcp-servers", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_array(), "top-mcp-servers should be an array");
}

#[tokio::test]
async fn test_conversations_list_and_cost_summary() {
    let (base_url, client) = setup().await;

    // Ingest a couple of events into one conversation.
    for i in 0..2 {
        let event = json!({
            "event_id": uuid::Uuid::new_v4().to_string(),
            "tool_name": format!("tool_{i}"),
            "agent": "cursor",
            "model": "gpt-4o",
            "conversation_id": "conv-list-test",
            "started_at": "2026-05-17T00:00:00Z",
            "ended_at": "2026-05-17T00:00:01Z",
            "ok": true
        });
        let resp = client
            .post(format!("{}/events/tool-call", base_url))
            .json(&event)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }

    let convs: serde_json::Value = client
        .get(format!("{}/api/conversations?limit=10", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(convs.is_array(), "conversations should be an array");

    let cost: serde_json::Value = client
        .get(format!("{}/api/cost/summary", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(cost.get("kpis").is_some(), "cost summary should have kpis");
}

#[tokio::test]
async fn test_admin_delete_and_reset() {
    let (base_url, client) = setup().await;

    let event = json!({
        "event_id": uuid::Uuid::new_v4().to_string(),
        "tool_name": "admin_test_tool",
        "conversation_id": "conv-admin-test",
        "started_at": "2026-05-17T00:00:00Z",
        "ended_at": "2026-05-17T00:00:01Z",
        "ok": true,
        "estimated_input_tokens": 1000,
        "estimated_output_tokens": 200,
        "model": "gpt-4o"
    });
    client
        .post(format!("{}/events/tool-call", base_url))
        .json(&event)
        .send()
        .await
        .unwrap();

    let del: serde_json::Value = client
        .delete(format!("{}/api/conversations/conv-admin-test", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(del["deleted_events"], 1);

    client
        .post(format!("{}/events/tool-call", base_url))
        .json(&event)
        .send()
        .await
        .unwrap();

    let reset: serde_json::Value = client
        .post(format!("{}/api/admin/reset", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(reset["deleted_events"].as_u64().unwrap_or(0) >= 1);

    let convs: serde_json::Value = client
        .get(format!("{}/api/conversations?limit=10", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(convs.as_array().map(|a| a.len()).unwrap_or(99), 0);
}

#[tokio::test]
async fn test_cost_summary_burn_rate_nonzero() {
    let (base_url, client) = setup().await;

    let event = json!({
        "event_id": uuid::Uuid::new_v4().to_string(),
        "tool_name": "cost_tool",
        "conversation_id": "conv-cost",
        "started_at": "2026-05-17T00:00:00Z",
        "ended_at": "2026-05-17T00:00:01Z",
        "ok": true,
        "estimated_input_tokens": 5000,
        "estimated_output_tokens": 1000,
        "model": "gpt-4o"
    });
    client
        .post(format!("{}/events/tool-call", base_url))
        .json(&event)
        .send()
        .await
        .unwrap();

    let cost: serde_json::Value = client
        .get(format!(
            "{}/api/cost/summary?from=2026-05-16T00:00:00Z&to=2026-05-18T00:00:00Z",
            base_url
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let burn = cost["kpis"]["burn_rate_usd_per_hour"]
        .as_f64()
        .unwrap_or(0.0);
    assert!(
        burn > 0.0,
        "burn rate should be computed from spend / hours"
    );
}
