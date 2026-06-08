use agent_meter_collector::{app, config::Config, db};
use agent_meter_db::{Database, PostgresDb};
use reqwest::Client;
use serde_json::json;
use std::sync::Arc;

async fn setup() -> (String, Client) {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://agent_meter:agent_meter@localhost:54321/agent_meter".into()
    });

    let pool = db::connect(&database_url).await.unwrap();
    let config = Config::from_env();
    let db: Arc<dyn Database> = Arc::new(PostgresDb::from_pool(pool.clone()));
    let app = app::build(config, pool, db);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    (base_url, Client::new())
}

#[tokio::test]
async fn test_health() {
    let (base_url, client) = setup().await;
    let resp = client.get(format!("{}/health", base_url)).send().await.unwrap();
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
    assert!(body.contains("<title>Agent Meter"));
}

#[tokio::test]
async fn test_post_tool_call_event() {
    let (base_url, client) = setup().await;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let task_id = format!("test-task-{}", ts);
    let event = json!({
        "event_id": format!("{:032x}", ts),
        "task_id": task_id,
        "task_id": "test-task-integration",
        "tool_name": "integration_test_tool",
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
async fn test_reports_top_tasks() {
    let (base_url, client) = setup().await;
    let resp = client
        .get(format!("{}/reports/top-tasks", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.is_array(), "top-tasks should be an array");
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
async fn test_reports_events_supports_cursor_pagination() {
    let (base_url, client) = setup().await;
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let conversation_id = format!("conv-cursor-test-{}", run_id);

    for (event_id, started_at, tool_name) in [
        (
            uuid::Uuid::new_v4().to_string(),
            "2026-05-17T00:00:03Z",
            "cursor_test_tool_3",
        ),
        (
            uuid::Uuid::new_v4().to_string(),
            "2026-05-17T00:00:02Z",
            "cursor_test_tool_2",
        ),
        (
            uuid::Uuid::new_v4().to_string(),
            "2026-05-17T00:00:01Z",
            "cursor_test_tool_1",
        ),
    ] {
        let event = json!({
            "event_id": event_id,
            "task_id": format!("events-cursor-test-{}", run_id),
            "tool_name": tool_name,
            "started_at": started_at,
            "ended_at": started_at,
            "ok": true,
            "request_bytes": 10,
            "response_bytes": 20,
            "conversation_id": conversation_id
        });
        let resp = client
            .post(format!("{}/events/tool-call", base_url))
            .json(&event)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "cursor event insert should succeed");
    }

    let first_page: serde_json::Value = client
        .get(format!(
            "{}/reports/events?conversation_id={}&limit=2",
            base_url, conversation_id
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let first_page = first_page.as_array().unwrap();
    assert_eq!(first_page.len(), 2);
    assert_eq!(first_page[0]["tool_name"], "cursor_test_tool_3");
    assert_eq!(first_page[1]["tool_name"], "cursor_test_tool_2");

    let before_started_at = first_page[1]["started_at"].as_str().unwrap();
    let before_event_id = first_page[1]["event_id"].as_str().unwrap();
    let second_page: serde_json::Value = client
        .get(format!(
            "{}/reports/events?conversation_id={}&limit=2&before_started_at={}&before_event_id={}",
            base_url,
            conversation_id,
            before_started_at,
            before_event_id
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let second_page = second_page.as_array().unwrap();
    assert_eq!(second_page.len(), 1);
    assert_eq!(second_page[0]["tool_name"], "cursor_test_tool_1");
}

#[tokio::test]
async fn test_tasks_start_end_list() {
    let (base_url, client) = setup().await;

    let start = json!({
        "task_id": "test-task-001",
        "repo": "test-repo",
        "branch": "feature-x",
        "ide": "test-ide",
        "agent": "test-agent",
        "skill": "integration"
    });
    let resp = client
        .post(format!("{}/tasks/start", base_url))
        .json(&start)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "task start");

    let resp = client
        .get(format!("{}/tasks", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let tasks: serde_json::Value = resp.json().await.unwrap();
    assert!(tasks.is_array());
    let has_task = tasks.as_array().unwrap().iter().any(|t| {
        t.get("task_id").and_then(|v| v.as_str()) == Some("test-task-001")
    });
    assert!(has_task, "task should appear in list");

    let end = json!({
        "task_id": "test-task-001",
        "ended_at": "2026-05-17T01:00:00Z"
    });
    let resp = client
        .post(format!("{}/tasks/end", base_url))
        .json(&end)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "task end");
}

#[tokio::test]
async fn test_tasks_end_non_existent() {
    let (base_url, client) = setup().await;
    let end = json!({
        "task_id": "task-that-does-not-exist"
    });
    let resp = client
        .post(format!("{}/tasks/end", base_url))
        .json(&end)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404, "end non-existent should 404");
}

#[tokio::test]
async fn test_billing_plans() {
    let (base_url, client) = setup().await;
    let resp = client
        .get(format!("{}/api/billing/plans", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "plans endpoint");
    let plans: serde_json::Value = resp.json().await.unwrap();
    let arr = plans.as_array().expect("plans is an array");
    // Free, Pro, Team, Enterprise.
    assert_eq!(arr.len(), 4, "should have 4 tiers");
    let ids: Vec<&str> = arr.iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["free", "pro", "team", "enterprise"]);
    // Pricing comes from the API, not hardcoded HTML.
    let pro = arr.iter().find(|p| p["id"] == "pro").unwrap();
    assert_eq!(pro["price"], 19);
    assert_eq!(pro["featured"], true);
    assert!(pro["features"].as_array().unwrap().len() >= 3);
    // Enterprise has no fixed price.
    let ent = arr.iter().find(|p| p["id"] == "enterprise").unwrap();
    assert!(ent["price"].is_null(), "enterprise price is Custom (null)");
}

#[tokio::test]
async fn test_billing_stub_redirects() {
    let (base_url, _client) = setup().await;
    // Use a non-following client to observe the redirect.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client
        .get(format!("{}/billing/stub", base_url))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_redirection(), "stub should redirect");
    let loc = resp.headers().get("location").unwrap().to_str().unwrap();
    assert_eq!(loc, "/pricing?mode=stub");
}
