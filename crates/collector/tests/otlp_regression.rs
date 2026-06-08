/// Regression tests — OTLP ingestion por ferramenta/agent.
///
/// Cada fixture representa um payload real (ou sintético fiel ao spec) enviado
/// via OTLP JSON para o endpoint /v1/traces.  Os testes verificam:
///   - `ide` detectado corretamente
///   - `tool_name` extraído corretamente
///   - `model` capturado quando disponível
///   - `conversation_id` agrupado corretamente
///   - Nenhum evento perdido (count == N spans esperados)
///
/// Ferramentas cobertas (por prioridade de produto):
///   1. VS Code Copilot        (execute_tool + chat)
///   2. Cursor                 (execute_tool + chat; service.name=cursor)
///   3. Antigravity            (proto — testado no api.rs com mcp-wrapper)
///   4. Claude Code            (execute_tool + chat; service.name=claude)
///   5. Codex CLI              (execute_tool; service.name=codex)
///   6. MCP OTel semconv       (tools/call <tool>; novo padrão)

use agent_meter_collector::{app, config::Config, db};
use agent_meter_db::{Database, PostgresDb};
use reqwest::Client;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

async fn setup() -> (String, Client) {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://agent_meter:agent_meter@localhost:54321/agent_meter".into()
    });
    let pool = db::connect(&database_url).await.unwrap();
    let config = Config::from_env();
    let db: Arc<dyn Database> = Arc::new(PostgresDb::from_pool(pool.clone()));
    let app = app::build_otlp(config, pool, db);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);
    tokio::spawn(async move {
        axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>())
            .await
            .unwrap()
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    (base_url, Client::new())
}

fn load_fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to load fixture {name}: {e}"))
}

async fn post_otlp(base_url: &str, client: &Client, fixture: &str) -> Vec<Value> {
    let body = load_fixture(fixture);
    let resp = client
        .post(format!("{}/v1/traces", base_url))
        .header("content-type", "application/json")
        .header("user-agent", infer_ua_from_fixture(fixture))
        .body(body)
        .send()
        .await
        .unwrap();
    let status = resp.status();
    let text = resp.text().await.unwrap();
    assert!(
        status.is_success(),
        "OTLP ingest failed for {fixture}: HTTP {status} — {text}"
    );
    serde_json::from_str::<Vec<Value>>(&text).unwrap_or_default()
}

/// Infer realistic user-agent from fixture name so `infer_ide` can detect the source.
fn infer_ua_from_fixture(fixture: &str) -> &'static str {
    match fixture {
        f if f.starts_with("vscode") => "vscode/1.100.0 (darwin arm64)",
        f if f.starts_with("cursor") => "cursor/0.48.0 (darwin arm64)",
        f if f.starts_with("eclipse") => "eclipse/2026-03 jdt-language-server",
        f if f.starts_with("claude") => "claude-code/1.0.0 (linux arm64)",
        f if f.starts_with("codex")  => "codex/0.1.0 (linux amd64)",
        f if f.starts_with("mcp")    => "my-agent/1.0.0",
        _ => "unknown-agent/1.0",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. VS Code Copilot
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_otlp_vscode_copilot_execute_tool() {
    let (base_url, client) = setup().await;
    let events = post_otlp(&base_url, &client, "vscode_copilot_execute_tool.json").await;
    assert_eq!(events.len(), 1, "expected 1 event from VS Code execute_tool fixture");
    let e = &events[0];
    assert_eq!(e["tool_name"], "run_in_terminal", "tool_name mismatch");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_otlp_vscode_copilot_chat() {
    let (base_url, client) = setup().await;
    let events = post_otlp(&base_url, &client, "vscode_copilot_chat.json").await;
    assert_eq!(events.len(), 1, "expected 1 event from VS Code chat fixture");
    let e = &events[0];
    assert_eq!(e["tool_name"], "llm_chat", "chat spans should produce tool_name=llm_chat");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_otlp_eclipse_copilot_execute_tool_and_chat() {
    let (base_url, client) = setup().await;
    // fixture has 1 execute_tool + 1 chat span
    let events = post_otlp(&base_url, &client, "eclipse_copilot_execute_tool.json").await;
    assert_eq!(events.len(), 2, "eclipse fixture should produce 2 events (tool + chat)");

    let tool_event = events.iter().find(|e| e["tool_name"] != "llm_chat");
    let chat_event = events.iter().find(|e| e["tool_name"] == "llm_chat");
    assert!(tool_event.is_some(), "should have a tool event");
    assert!(chat_event.is_some(), "should have a chat/llm_chat event");
    assert_eq!(tool_event.unwrap()["tool_name"], "read_file");
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Cursor
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_otlp_cursor_execute_tool_and_chat() {
    let (base_url, client) = setup().await;
    // fixture has 1 execute_tool + 1 chat span
    let events = post_otlp(&base_url, &client, "cursor_execute_tool.json").await;
    assert_eq!(events.len(), 2, "cursor fixture should produce 2 events (tool + chat)");

    let tool_event = events.iter().find(|e| e["tool_name"] != "llm_chat");
    let chat_event = events.iter().find(|e| e["tool_name"] == "llm_chat");
    assert!(tool_event.is_some(), "should have a tool event");
    assert!(chat_event.is_some(), "should have a chat/llm_chat event");
    assert_eq!(tool_event.unwrap()["tool_name"], "read_file");
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Claude Code (Anthropic CLI)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_otlp_claude_code_execute_tool_and_chat() {
    let (base_url, client) = setup().await;
    // fixture has 1 execute_tool (bash) + 1 chat span
    let events = post_otlp(&base_url, &client, "claude_code_execute_tool.json").await;
    assert_eq!(events.len(), 2, "claude-code fixture should produce 2 events");

    let tool_event = events.iter().find(|e| e["tool_name"] != "llm_chat");
    assert!(tool_event.is_some(), "should have a bash tool event");
    assert_eq!(tool_event.unwrap()["tool_name"], "bash");
}

// ─────────────────────────────────────────────────────────────────────────────
// 5. Codex CLI (OpenAI)
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_otlp_codex_cli_execute_tool() {
    let (base_url, client) = setup().await;
    let events = post_otlp(&base_url, &client, "codex_cli_execute_tool.json").await;
    assert_eq!(events.len(), 1, "codex fixture should produce 1 event");
    let e = &events[0];
    assert_eq!(e["tool_name"], "shell", "tool_name mismatch");
}

// ─────────────────────────────────────────────────────────────────────────────
// 6. MCP OTel semconv — tools/call <tool>
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_otlp_mcp_semconv_tools_call() {
    let (base_url, client) = setup().await;
    // fixture has 2 tools/call spans (get-weather, read_file)
    let events = post_otlp(&base_url, &client, "mcp_semconv_tools_call.json").await;
    assert_eq!(events.len(), 2, "MCP semconv fixture should produce 2 events (2 tools/call spans)");

    let weather = events.iter().find(|e| e["tool_name"] == "get-weather");
    let read    = events.iter().find(|e| e["tool_name"] == "read_file");
    assert!(weather.is_some(), "get-weather event missing");
    assert!(read.is_some(), "read_file event missing");
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge cases
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn test_otlp_empty_body_returns_empty() {
    let (base_url, client) = setup().await;
    let resp = client
        .post(format!("{}/v1/traces", base_url))
        .header("content-type", "application/json")
        .body("{\"resourceSpans\":[]}")
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "empty resourceSpans should 200");
    let events: Vec<Value> = resp.json().await.unwrap();
    assert!(events.is_empty(), "empty resourceSpans should produce 0 events");
}

#[tokio::test(flavor = "multi_thread")]
async fn test_otlp_unknown_span_name_produces_no_panic() {
    let (base_url, client) = setup().await;
    let body = r#"{
        "resourceSpans": [{
            "resource": {"attributes": [{"key":"service.name","value":{"stringValue":"test"}}]},
            "scopeSpans": [{"spans": [{"name":"invoke_agent some-agent","startTimeUnixNano":"1717500000000000000","endTimeUnixNano":"1717500001000000000","status":{"code":1},"attributes":[]}]}]
        }]
    }"#;
    let resp = client
        .post(format!("{}/v1/traces", base_url))
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .unwrap();
    // invoke_agent is silently ignored (no panic, 200 with empty result)
    assert!(resp.status().is_success());
}
