use agent_meter_collector::{app, config::Config, db};
use reqwest::Client;
use serde_json::json;

async fn setup() -> (String, Client) {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://agent_meter:agent_meter@localhost:5433/agent_meter".into()
    });

    let pool = db::connect(&database_url).await.unwrap();
    let config = Config::from_env();
    let app = app::build(config, pool);

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
    assert!(body.contains("agent-meter dashboard"));
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
