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
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
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

async fn seed_filter_fixtures(base_url: &str, client: &Client) {
    let fixtures = [
        (
            "read_file",
            "cursor",
            "composer",
            "agent-meter",
            "refactor",
            "conv-filter-a",
        ),
        (
            "grep",
            "cursor",
            "composer",
            "agent-meter",
            "refactor",
            "conv-filter-a",
        ),
        (
            "run_terminal",
            "vscode",
            "copilot",
            "other-repo",
            "debug",
            "conv-filter-b",
        ),
    ];

    for (tool, ide, agent, repo, skill, conversation_id) in fixtures {
        let event = json!({
            "event_id": uuid::Uuid::new_v4().to_string(),
            "tool_name": tool,
            "ide": ide,
            "agent": agent,
            "repo": repo,
            "skill": skill,
            "conversation_id": conversation_id,
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

    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
}

#[tokio::test]
async fn test_reports_empty_dataset_returns_stable_arrays() {
    let (base_url, client) = setup().await;

    for path in [
        "/reports/top-tools",
        "/reports/top-agents",
        "/reports/top-mcp-servers",
        "/reports/events?limit=5",
        "/api/conversations?limit=5",
    ] {
        let resp = client
            .get(format!("{base_url}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "path {path}");
        let body: serde_json::Value = resp.json().await.unwrap();
        assert!(body.is_array(), "{path} should return array");
        assert_eq!(body.as_array().unwrap().len(), 0, "{path} should be empty");
    }
}

#[tokio::test]
async fn test_events_feed_pagination_and_filters() {
    let (base_url, client) = setup().await;
    seed_filter_fixtures(&base_url, &client).await;

    let page1: serde_json::Value = client
        .get(format!("{}/reports/events?limit=2&offset=0", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let page2: serde_json::Value = client
        .get(format!("{}/reports/events?limit=2&offset=2", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(page1.as_array().unwrap().len(), 2);
    assert_eq!(page2.as_array().unwrap().len(), 1);

    let filtered: serde_json::Value = client
        .get(format!(
            "{}/reports/events?ide=vscode&agent=copilot&limit=10",
            base_url
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let rows = filtered.as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["tool_name"], "run_terminal");
    assert_eq!(rows[0]["ide"], "vscode");
}

#[tokio::test]
async fn test_report_filters_by_repo_and_skill() {
    let (base_url, client) = setup().await;
    seed_filter_fixtures(&base_url, &client).await;

    let tools: serde_json::Value = client
        .get(format!(
            "{}/reports/top-tools?repo=agent-meter&skill=refactor&limit=10",
            base_url
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let rows = tools.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row["calls"].as_i64() == Some(1)));

    let agents: serde_json::Value = client
        .get(format!(
            "{}/reports/top-agents?ide=cursor&limit=10",
            base_url
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(agents.as_array().unwrap().len(), 1);
    assert_eq!(agents[0]["agent"], "composer");
    assert_eq!(agents[0]["calls"], 2);
}

#[tokio::test]
async fn test_conversations_pagination_and_empty_page() {
    let (base_url, client) = setup().await;
    seed_filter_fixtures(&base_url, &client).await;

    let page: serde_json::Value = client
        .get(format!("{}/api/conversations?limit=1&offset=0", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page.as_array().unwrap().len(), 1);

    let empty_page: serde_json::Value = client
        .get(format!("{}/api/conversations?limit=10&offset=99", base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(empty_page.as_array().unwrap().is_empty());

    let filtered: serde_json::Value = client
        .get(format!(
            "{}/api/conversations?ide=vscode&limit=10",
            base_url
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(filtered.as_array().unwrap().len(), 1);
    assert_eq!(filtered[0]["conversation_id"], "conv-filter-b");
}

#[tokio::test]
async fn test_docs_and_static_assets_smoke() {
    let (base_url, client) = setup().await;

    let html_routes = ["/docs", "/conversations", "/reports", "/cost"];
    for path in html_routes {
        let resp = client
            .get(format!("{base_url}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "route {path}");
        let body = resp.text().await.unwrap();
        assert!(body.starts_with("<!DOCTYPE html>"), "{path} should be html");
        assert!(body.contains("/_static/app.css"), "{path} should link css");
        assert!(body.contains("/_static/app.js"), "{path} should link js");
    }

    let assets = [
        ("/_static/tokens.css", "text/css"),
        ("/_static/app.css", "text/css"),
        ("/_static/app.js", "application/javascript"),
        ("/_static/icons.svg", "image/svg+xml"),
        ("/_static/favicon.svg", "image/svg+xml"),
    ];
    for (path, content_type) in assets {
        let resp = client
            .get(format!("{base_url}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "asset {path}");
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ct.contains(content_type),
            "asset {path} content-type expected {content_type}, got {ct}"
        );
        let body = resp.text().await.unwrap();
        assert!(!body.is_empty(), "asset {path} should not be empty");
    }
}

#[tokio::test]
async fn test_embed_badges_svg() {
    let (base_url, client) = setup().await;

    for path in ["/badge/cost.svg", "/badge/events.svg"] {
        let resp = client
            .get(format!("{base_url}{path}"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "badge {path}");
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(ct.contains("image/svg+xml"), "badge content-type: {ct}");
        let body = resp.text().await.unwrap();
        assert!(body.contains("<svg"), "badge {path} should be svg");
        assert!(body.contains("<title>"), "badge {path} should have title");
    }
}

#[tokio::test]
async fn test_api_key_auth_when_required() {
    use agent_meter_collector::services::auth;

    let db = make_db().await;
    let org = db.find_org_by_slug("personal").await.expect("personal org");
    let secret = format!("am_live_{}", uuid::Uuid::new_v4().as_simple());
    let prefix = &secret[..12];
    db.create_api_key(org.id, "test", prefix, &auth::hash_key(&secret))
        .await
        .expect("create api key");

    let mut config = agent_meter_collector::config::Config::from_env();
    config.require_api_key = true;
    let app = app::build(config, db, CancellationToken::new());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    let client = Client::new();

    let event = json!({
        "event_id": uuid::Uuid::new_v4().to_string(),
        "tool_name": "auth_test_tool",
        "conversation_id": "conv-auth-test",
        "started_at": "2026-05-17T00:00:00Z",
        "ended_at": "2026-05-17T00:00:01Z",
        "ok": true
    });

    let denied = client
        .post(format!("{}/events/tool-call", base_url))
        .json(&event)
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 401);

    let ok = client
        .post(format!("{}/events/tool-call", base_url))
        .header("Authorization", format!("Bearer {}", secret))
        .json(&event)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
}
