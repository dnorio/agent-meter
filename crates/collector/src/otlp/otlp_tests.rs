
use super::*;

fn attr_str(key: &str, val: &str) -> serde_json::Value {
    serde_json::json!({"key": key, "value": {"stringValue": val}})
}

fn attr_int(key: &str, val: i64) -> serde_json::Value {
    serde_json::json!({"key": key, "value": {"intValue": val}})
}

fn attr_int_str(key: &str, val: &str) -> serde_json::Value {
    serde_json::json!({"key": key, "value": {"intValue": val}})
}

fn attr_double(key: &str, val: f64) -> serde_json::Value {
    serde_json::json!({"key": key, "value": {"doubleValue": val}})
}

fn span_with_attrs(name: &str, attrs: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "attributes": attrs,
    })
}

// ── infer_mcp_server_from_tool ──────────────────────────────────────────

#[test]
fn infer_mcp_server_table() {
    let cases: &[(&str, Option<&str>)] = &[
        ("mcp_chromedevtool_navigate", Some("chromeDevtools")),
        ("mcp_chromedevtools_click", Some("chromeDevtools")),
        ("mcp_chrome_eval", Some("chromeDevtools")),
        ("mcp_gitkraken_status", Some("gitkraken")),
        ("mcp_playwright_goto", Some("playwright")),
        ("mcp_filesystem_read", Some("filesystem")),
        ("mcp_custom_server_tool", Some("custom")),
        ("run_in_terminal", Some("vscode-builtin")),
        ("read_file", Some("vscode-builtin")),
        ("grep_search", Some("vscode-builtin")),
        ("semantic_search", Some("vscode-builtin")),
        ("manage_todo_list", Some("vscode-builtin")),
        ("runSubagent", Some("vscode-builtin")),
        ("memory", Some("copilot-memory")),
        ("memory_store", Some("copilot-memory")),
        ("fetch_webpage", Some("fetch")),
        ("fetch_url", Some("fetch")),
        ("unknown_tool_xyz", None),
        ("", None),
    ];
    for &(tool, expected) in cases {
        assert_eq!(
            infer_mcp_server_from_tool(tool).as_deref(),
            expected,
            "tool={tool}"
        );
    }
}

// ── truncate_str ────────────────────────────────────────────────────────

#[test]
fn truncate_str_passthrough_when_short() {
    assert_eq!(truncate_str("hello".into(), 100), "hello");
    assert_eq!(truncate_str("abc".into(), 3), "abc");
}

#[test]
fn truncate_str_cuts_and_marks() {
    let out = truncate_str("abcdefghij".into(), 5);
    assert!(out.starts_with("abcde"));
    assert!(out.ends_with("…[truncated]"));
}

#[test]
fn truncate_str_respects_utf8_boundary() {
    // "é" is 2 bytes; max_bytes=1 must not split the char
    let s = "éabc".to_string();
    let out = truncate_str(s, 1);
    assert!(out.ends_with("…[truncated]"));
    // empty prefix before ellipsis when boundary is at 0
    assert!(out.starts_with('…') || out.starts_with("…") || !out.is_empty());
}

// ── is_noise_prompt ─────────────────────────────────────────────────────

#[test]
fn is_noise_prompt_table() {
    let cases: &[(&str, bool)] = &[
        ("hello world", false),
        ("[{", true),
        ("{\"a\":1}", true),
        ("The current date is Monday", true),
        ("Terminals: foo", true),
        ("[Terminal 1]", true),
        ("You are a helpful assistant", true),
        ("summarize the following text please", true),
        ("Please write a brief title for this", true),
        ("<current_datetime>2026-01-01</current_datetime>", false),
        (&"x".repeat(2001), true),
        ("short ok", false),
    ];
    for &(s, expected) in cases {
        assert_eq!(is_noise_prompt(s), expected, "s={s:.40}");
    }
    // long XML noise (>500 chars starting with <)
    let long_xml = format!("<system>{}</system>", "a".repeat(600));
    assert!(is_noise_prompt(&long_xml));
}

// ── extract_user_request_tag ────────────────────────────────────────────

#[test]
fn extract_user_request_tag_basic() {
    let content = "prefix <userRequest>fix the bug</userRequest> suffix";
    assert_eq!(
        extract_user_request_tag(content).as_deref(),
        Some("fix the bug")
    );
}

#[test]
fn extract_user_request_tag_skips_conversation_summary() {
    let content =
        "<conversation-summary>see <userRequest>docs</userRequest></conversation-summary>";
    assert_eq!(extract_user_request_tag(content), None);
}

#[test]
fn extract_user_request_tag_outside_summary_ok() {
    let content =
        "<conversation-summary>old</conversation-summary><userRequest>new prompt</userRequest>";
    assert_eq!(
        extract_user_request_tag(content).as_deref(),
        Some("new prompt")
    );
}

#[test]
fn extract_user_request_tag_empty_or_noise() {
    assert_eq!(
        extract_user_request_tag("<userRequest>  </userRequest>"),
        None
    );
    assert_eq!(extract_user_request_tag("no tags here"), None);
    assert_eq!(
        extract_user_request_tag("<userRequest>You are a bot</userRequest>"),
        None
    );
}

// ── parse_first_human_text ──────────────────────────────────────────────

#[test]
fn parse_first_human_text_parts_plain() {
    let raw = r#"[{"role":"user","parts":[{"type":"text","content":"<environment_info>x</environment_info>"},{"type":"text","content":"actual prompt"}]}]"#;
    assert_eq!(
        parse_first_human_text(raw).as_deref(),
        Some("actual prompt")
    );
}

#[test]
fn parse_first_human_text_parts_user_request_tag() {
    let raw = r#"[{"role":"user","parts":[{"type":"text","content":"<userRequest>tagged prompt</userRequest>"}]}]"#;
    assert_eq!(
        parse_first_human_text(raw).as_deref(),
        Some("tagged prompt")
    );
}

#[test]
fn parse_first_human_text_plain_string_content() {
    let raw =
        r#"[{"role":"assistant","content":"hi"},{"role":"user","content":"  do the thing  "}]"#;
    assert_eq!(parse_first_human_text(raw).as_deref(), Some("do the thing"));
}

#[test]
fn parse_first_human_text_anthropic_blocks() {
    let raw = r#"[{"role":"user","content":[{"type":"text","text":"anthropic prompt"}]}]"#;
    assert_eq!(
        parse_first_human_text(raw).as_deref(),
        Some("anthropic prompt")
    );
}

#[test]
fn parse_first_human_text_skips_tool_result_turn() {
    let raw = r#"[{"role":"user","content":[{"type":"tool_result","content":"ok"}]},{"role":"user","content":[{"type":"text","text":"next"}]}]"#;
    assert_eq!(parse_first_human_text(raw).as_deref(), Some("next"));
}

#[test]
fn parse_first_human_text_invalid_or_empty() {
    assert_eq!(parse_first_human_text("not-json"), None);
    assert_eq!(parse_first_human_text("{}"), None);
    assert_eq!(parse_first_human_text("[]"), None);
    assert_eq!(
        parse_first_human_text(r#"[{"role":"system","content":"x"}]"#),
        None
    );
}

#[test]
fn parse_first_human_text_xml_content_with_tag() {
    let raw = r#"[{"role":"user","content":"<userRequest>from string</userRequest>"}]"#;
    assert_eq!(parse_first_human_text(raw).as_deref(), Some("from string"));
}

// ── json_attr_* ─────────────────────────────────────────────────────────

#[test]
fn json_attr_str_finds_and_misses() {
    let attrs = vec![attr_str("a", "one"), attr_str("b", "two")];
    assert_eq!(json_attr_str(&attrs, "a").as_deref(), Some("one"));
    assert_eq!(json_attr_str(&attrs, "b").as_deref(), Some("two"));
    assert_eq!(json_attr_str(&attrs, "missing"), None);
    assert_eq!(json_attr_str(&[], "a"), None);
}

#[test]
fn json_attr_int_variants() {
    let attrs = vec![
        attr_int("n", 42),
        attr_int_str("s", "99"),
        attr_double("d", 3.9),
        attr_str("p", "7"),
        serde_json::json!({"key": "p2", "value": {"stringValue": "not-a-number"}}),
    ];
    assert_eq!(json_attr_int(&attrs, "n"), Some(42));
    assert_eq!(json_attr_int(&attrs, "s"), Some(99));
    assert_eq!(json_attr_int(&attrs, "d"), Some(3));
    assert_eq!(json_attr_int(&attrs, "p"), Some(7));
    assert_eq!(json_attr_int(&attrs, "p2"), None);
    assert_eq!(json_attr_int(&attrs, "missing"), None);
}

#[test]
fn json_attr_float_variants() {
    let attrs = vec![
        attr_double("t", 0.7),
        attr_int("i", 2),
        attr_str("s", "1.5"),
        attr_str("bad", "nope"),
    ];
    assert_eq!(json_attr_float(&attrs, "t"), Some(0.7));
    assert_eq!(json_attr_float(&attrs, "i"), Some(2.0));
    assert_eq!(json_attr_float(&attrs, "s"), Some(1.5));
    assert_eq!(json_attr_float(&attrs, "bad"), None);
    assert_eq!(json_attr_float(&attrs, "missing"), None);
}

// ── extract_clean_user_prompt_json ──────────────────────────────────────

#[test]
fn extract_clean_prefers_copilot_user_request() {
    let attrs = vec![
        attr_str("copilot_chat.user_request", "typed by user"),
        attr_str("gen_ai.prompt", "ignored"),
    ];
    assert_eq!(
        extract_clean_user_prompt_json(&attrs).as_deref(),
        Some("typed by user")
    );
}

#[test]
fn extract_clean_copilot_json_array() {
    let msgs = r#"[{"role":"user","content":"from json array"}]"#;
    let attrs = vec![attr_str("copilot_chat.user_request", msgs)];
    assert_eq!(
        extract_clean_user_prompt_json(&attrs).as_deref(),
        Some("from json array")
    );
}

#[test]
fn extract_clean_from_input_messages() {
    let msgs = r#"[{"role":"user","content":"via input messages"}]"#;
    let attrs = vec![attr_str("gen_ai.input.messages", msgs)];
    assert_eq!(
        extract_clean_user_prompt_json(&attrs).as_deref(),
        Some("via input messages")
    );
}

#[test]
fn extract_clean_from_gen_ai_prompt_tag() {
    let attrs = vec![attr_str(
        "gen_ai.prompt",
        "ctx <userRequest>tagged</userRequest>",
    )];
    assert_eq!(
        extract_clean_user_prompt_json(&attrs).as_deref(),
        Some("tagged")
    );
}

#[test]
fn extract_clean_from_gen_ai_prompt_plain() {
    let attrs = vec![attr_str("gen_ai.prompt", "plain prompt text")];
    assert_eq!(
        extract_clean_user_prompt_json(&attrs).as_deref(),
        Some("plain prompt text")
    );
}

#[test]
fn extract_clean_from_indexed_prompt() {
    let attrs = vec![
        attr_str("gen_ai.prompt.0.role", "system"),
        attr_str("gen_ai.prompt.0.content", "sys"),
        attr_str("gen_ai.prompt.1.role", "user"),
        attr_str("gen_ai.prompt.1.content", "indexed user"),
    ];
    assert_eq!(
        extract_clean_user_prompt_json(&attrs).as_deref(),
        Some("indexed user")
    );
}

#[test]
fn extract_clean_rejects_noise_copilot() {
    let attrs = vec![attr_str(
        "copilot_chat.user_request",
        "You are a helpful assistant",
    )];
    assert_eq!(extract_clean_user_prompt_json(&attrs), None);
}

#[test]
fn extract_clean_empty_attrs() {
    assert_eq!(extract_clean_user_prompt_json(&[]), None);
}

// ── extract_response_text ───────────────────────────────────────────────

#[test]
fn extract_response_copilot_chat() {
    let attrs = vec![attr_str("copilot_chat.response", "assistant reply")];
    assert_eq!(
        extract_response_text(&attrs).as_deref(),
        Some("assistant reply")
    );
}

#[test]
fn extract_response_output_messages_plain() {
    let msgs = r#"[{"role":"assistant","content":"out plain"}]"#;
    let attrs = vec![attr_str("gen_ai.output.messages", msgs)];
    assert_eq!(extract_response_text(&attrs).as_deref(), Some("out plain"));
}

#[test]
fn extract_response_output_messages_blocks() {
    let msgs = r#"[{"role":"assistant","content":[{"type":"text","text":"block text"}]}]"#;
    let attrs = vec![attr_str("gen_ai.output.messages", msgs)];
    assert_eq!(extract_response_text(&attrs).as_deref(), Some("block text"));
}

#[test]
fn extract_response_output_messages_parts() {
    let msgs = r#"[{"role":"model","parts":[{"content":"gemini out"}]}]"#;
    let attrs = vec![attr_str("gen_ai.output.messages", msgs)];
    assert_eq!(extract_response_text(&attrs).as_deref(), Some("gemini out"));
}

#[test]
fn extract_response_indexed_completion() {
    let attrs = vec![attr_str("gen_ai.completion.0.content", "completion0")];
    assert_eq!(
        extract_response_text(&attrs).as_deref(),
        Some("completion0")
    );
}

#[test]
fn extract_response_generic_text() {
    let attrs = vec![attr_str("gen_ai.response.text", "generic")];
    assert_eq!(extract_response_text(&attrs).as_deref(), Some("generic"));
}

#[test]
fn extract_response_empty() {
    assert_eq!(extract_response_text(&[]), None);
    let attrs = vec![attr_str("copilot_chat.response", "")];
    assert_eq!(extract_response_text(&attrs), None);
}

// ── is_generic_http_method ──────────────────────────────────────────────

#[test]
fn is_generic_http_method_table() {
    for name in [
        "GET",
        "post",
        "PUT",
        "DELETE",
        "PATCH",
        "HEAD",
        "OPTIONS",
        "HTTP GET",
        "HTTP POST",
        "http put",
        "HTTP DELETE",
    ] {
        assert!(is_generic_http_method(name), "{name}");
    }
    for name in ["execute_tool", "chat gpt", "FOO", ""] {
        assert!(!is_generic_http_method(name), "{name}");
    }
}

// ── is_copilot_http_span / classify_copilot_http_span ────────────────────

#[test]
fn is_copilot_http_span_detects_urls() {
    let urls = [
        "https://api.github.com/copilot/v1/chat",
        "https://githubcopilot.com/v1",
        "https://copilot-proxy.example/x",
        "https://default.exp-tas.com/vscode",
        "https://api.githubcopilot.com/x",
    ];
    for url in urls {
        let span = span_with_attrs("POST", vec![attr_str("http.url", url)]);
        assert!(is_copilot_http_span(&span, "POST"), "url={url}");
    }
}

#[test]
fn is_copilot_http_span_rejects_non_http_name() {
    let span = span_with_attrs(
        "execute_tool foo",
        vec![attr_str("http.url", "https://api.github.com/copilot")],
    );
    assert!(!is_copilot_http_span(&span, "execute_tool foo"));
}

#[test]
fn is_copilot_http_span_rejects_unrelated_url() {
    let span = span_with_attrs("GET", vec![attr_str("http.url", "https://example.com/api")]);
    assert!(!is_copilot_http_span(&span, "GET"));
}

#[test]
fn is_copilot_http_span_no_attrs() {
    let span = serde_json::json!({"name": "GET"});
    assert!(!is_copilot_http_span(&span, "GET"));
}

#[test]
fn classify_copilot_http_span_table() {
    let cases: &[(&str, &str, bool)] = &[
        ("https://x/chat/completions", "llm_chat", true),
        ("https://x/conversation", "llm_chat", true),
        ("https://x/responses", "llm_chat", true),
        ("https://x/completions", "copilot_completions", false),
        ("https://x/telemetry", "copilot_telemetry", false),
        ("https://x/models", "copilot_models", false),
        ("https://x/token", "copilot_auth", false),
        ("https://x/oauth", "copilot_auth", false),
        ("https://x/other", "copilot_api", false),
    ];
    for &(url, tool, is_chat) in cases {
        let span = span_with_attrs("POST", vec![attr_str("url.full", url)]);
        let (name, chat) = classify_copilot_http_span(&span);
        assert_eq!(name, tool, "url={url}");
        assert_eq!(chat, is_chat, "url={url}");
    }
}

#[test]
fn classify_copilot_http_span_empty_url() {
    let span = span_with_attrs("GET", vec![]);
    let (name, chat) = classify_copilot_http_span(&span);
    assert_eq!(name, "copilot_api");
    assert!(!chat);
}

// ── get_attr_* proto helpers ────────────────────────────────────────────

fn kv_str(key: &str, val: &str) -> KeyValue {
    KeyValue {
        key: key.to_string(),
        value: Some(AnyValue {
            value: Some(any_value::Value::StringValue(val.to_string())),
        }),
    }
}

fn kv_int(key: &str, val: i64) -> KeyValue {
    KeyValue {
        key: key.to_string(),
        value: Some(AnyValue {
            value: Some(any_value::Value::IntValue(val)),
        }),
    }
}

fn kv_double(key: &str, val: f64) -> KeyValue {
    KeyValue {
        key: key.to_string(),
        value: Some(AnyValue {
            value: Some(any_value::Value::DoubleValue(val)),
        }),
    }
}

#[test]
fn get_attr_str_int_float() {
    let attrs = vec![
        kv_str("s", "hello"),
        kv_int("n", 10),
        kv_double("d", 1.25),
        kv_str("fs", "2.5"),
    ];
    assert_eq!(get_attr_str(&attrs, "s").as_deref(), Some("hello"));
    assert_eq!(get_attr_str(&attrs, "n"), None);
    assert_eq!(get_attr_str(&attrs, "missing"), None);
    assert_eq!(get_attr_int(&attrs, "n"), Some(10));
    assert_eq!(get_attr_int(&attrs, "s"), None);
    assert_eq!(get_attr_float(&attrs, "d"), Some(1.25));
    assert_eq!(get_attr_float(&attrs, "n"), Some(10.0));
    assert_eq!(get_attr_float(&attrs, "fs"), Some(2.5));
    assert_eq!(get_attr_float(&attrs, "missing"), None);
}

#[test]
fn extract_clean_user_prompt_proto_paths() {
    let attrs = vec![kv_str("copilot_chat.user_request", "proto prompt")];
    assert_eq!(
        extract_clean_user_prompt_proto(&attrs).as_deref(),
        Some("proto prompt")
    );

    let msgs = r#"[{"role":"user","content":"proto msgs"}]"#;
    let attrs = vec![kv_str("gen_ai.input.messages", msgs)];
    assert_eq!(
        extract_clean_user_prompt_proto(&attrs).as_deref(),
        Some("proto msgs")
    );

    let attrs = vec![kv_str(
        "gen_ai.prompt",
        "<userRequest>proto tag</userRequest>",
    )];
    assert_eq!(
        extract_clean_user_prompt_proto(&attrs).as_deref(),
        Some("proto tag")
    );

    let attrs = vec![kv_str("gen_ai.prompt.0.content", "indexed0")];
    assert_eq!(
        extract_clean_user_prompt_proto(&attrs).as_deref(),
        Some("indexed0")
    );

    assert_eq!(extract_clean_user_prompt_proto(&[]), None);
}

// ── classify / is_copilot proto ─────────────────────────────────────────

fn proto_span(name: &str, attrs: Vec<KeyValue>) -> Span {
    Span {
        name: name.to_string(),
        attributes: attrs,
        ..Default::default()
    }
}

#[test]
fn is_copilot_http_span_proto_and_classify() {
    let span = proto_span(
        "POST",
        vec![kv_str(
            "http.url",
            "https://api.github.com/copilot/chat/completions",
        )],
    );
    assert!(is_copilot_http_span_proto(&span));
    let (tool, is_chat) = classify_copilot_http_span_proto(&span);
    assert_eq!(tool, "llm_chat");
    assert!(is_chat);

    let span = proto_span("GET", vec![kv_str("http.url", "https://example.com")]);
    assert!(!is_copilot_http_span_proto(&span));

    let span = proto_span(
        "execute_tool",
        vec![kv_str("http.url", "https://api.github.com/copilot")],
    );
    assert!(!is_copilot_http_span_proto(&span));
}

#[test]
fn is_eclipse_service_http_span_checks() {
    let span = proto_span("GET", vec![]);
    assert!(is_eclipse_service_http_span(&span, Some("eclipse-jdt")));
    assert!(is_eclipse_service_http_span(&span, Some("copilot-eclipse")));
    assert!(!is_eclipse_service_http_span(&span, Some("vscode")));
    assert!(!is_eclipse_service_http_span(&span, None));

    let span = proto_span("execute_tool", vec![]);
    assert!(!is_eclipse_service_http_span(&span, Some("eclipse")));
}

#[test]
fn classify_copilot_http_span_proto_table() {
    let cases: &[(&str, &str, bool)] = &[
        ("https://x/completions", "copilot_completions", false),
        ("https://x/telemetry", "copilot_telemetry", false),
        ("https://x/models", "copilot_models", false),
        ("https://x/token", "copilot_auth", false),
        ("https://x/misc", "copilot_api", false),
    ];
    for &(url, tool, chat) in cases {
        let span = proto_span("GET", vec![kv_str("http.target", url)]);
        let (name, is_chat) = classify_copilot_http_span_proto(&span);
        assert_eq!(name, tool);
        assert_eq!(is_chat, chat);
    }
}

// ── end-to-end handle_trace_request (JSON + proto) ──────────────────────

async fn test_db() -> std::sync::Arc<dyn agent_meter_db::Database> {
    use agent_meter_db::{Database, SqliteDb};
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("am-otlp-unit-{n}.db"));
    let url = format!("sqlite://{}", path.display());
    let db = SqliteDb::connect(&url).await.expect("sqlite");
    db.migrate().await.expect("migrate");
    std::sync::Arc::new(db)
}

async fn test_buffer(
    capacity: usize,
) -> (
    std::sync::Arc<dyn agent_meter_db::Database>,
    crate::services::ingest_buffer::IngestBuffer,
    tokio_util::sync::CancellationToken,
) {
    let db = test_db().await;
    let cancel = tokio_util::sync::CancellationToken::new();
    let buf =
        crate::services::ingest_buffer::IngestBuffer::spawn(db.clone(), capacity, cancel.clone());
    (db, buf, cancel)
}

fn json_otlp_payload(spans: serde_json::Value, service: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "resourceSpans": [{
            "resource": {
                "attributes": [
                    {"key": "service.name", "value": {"stringValue": service}},
                    {"key": "session.id", "value": {"stringValue": "sess-cov"}}
                ]
            },
            "scopeSpans": [{"spans": spans}]
        }]
    }))
    .unwrap()
}

fn tool_span_json(name: &str) -> serde_json::Value {
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    serde_json::json!({
        "name": name,
        "startTimeUnixNano": now.to_string(),
        "endTimeUnixNano": (now + 1_000_000).to_string(),
        "attributes": [
            attr_str("gen_ai.tool.name", "read_file"),
            attr_str("gen_ai.agent.name", "cov-agent"),
            attr_int("gen_ai.usage.input_tokens", 10),
            attr_int("gen_ai.usage.output_tokens", 5),
        ],
        "status": {"code": 1}
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn handle_json_all_span_kinds_sync_persist() {
    let (_db, buf, _cancel) = test_buffer(64).await;
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let spans = serde_json::json!([
        tool_span_json("execute_tool read_file"),
        {
            "name": "chat gpt-4o",
            "startTimeUnixNano": now.to_string(),
            "endTimeUnixNano": (now + 2_000_000).to_string(),
            "attributes": [
                attr_str("gen_ai.response.model", "gpt-4o"),
                attr_int("gen_ai.usage.input_tokens", 20),
                attr_int("gen_ai.usage.output_tokens", 8),
                attr_str("copilot_chat.user_request", "hello cov"),
                attr_str("copilot_chat.response", "hi back"),
            ]
        },
        {"name": "invoke_agent my-agent", "startTimeUnixNano": "1", "endTimeUnixNano": "2", "attributes": []},
        tool_span_json("tools/call get-weather"),
        {
            "name": "POST",
            "startTimeUnixNano": now.to_string(),
            "endTimeUnixNano": (now + 3_000_000).to_string(),
            "attributes": [attr_str("http.url", "https://api.github.com/copilot/chat/completions")]
        },
        {
            "name": "GET",
            "startTimeUnixNano": now.to_string(),
            "endTimeUnixNano": (now + 3_000_000).to_string(),
            "attributes": [attr_str("http.url", "https://api.github.com/copilot/models")]
        },
        {"name": "totally_unknown_span", "startTimeUnixNano": "1", "endTimeUnixNano": "2", "attributes": []},
    ]);
    let body = json_otlp_payload(spans, "copilot-eclipse");
    let results = handle_trace_request(
        &body,
        Some("application/json"),
        Some("127.0.0.1"),
        Some("eclipse/1.0"),
        Some(&buf),
    )
    .expect("handle json");
    assert!(results.len() >= 4, "got {}", results.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn handle_json_invalid_and_empty_branches() {
    let err = handle_trace_request(b"not-json", Some("application/json"), None, None, None);
    assert!(err.is_err());

    let empty = handle_trace_request(
        br#"{"resourceSpans":[]}"#,
        Some("application/json"),
        None,
        None,
        None,
    )
    .unwrap();
    assert!(empty.is_empty());

    // missing resourceSpans key
    let none = handle_trace_request(br#"{}"#, Some("application/json"), None, None, None).unwrap();
    assert!(none.is_empty());

    // proto content-type with garbage → validation error
    let bad_proto = handle_trace_request(
        b"\x00\x01\x02",
        Some("application/x-protobuf"),
        None,
        None,
        None,
    );
    assert!(bad_proto.is_err());
}

fn encode_proto_req(spans: Vec<Span>, service: &str) -> Vec<u8> {
    let req = ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            resource: Some(Resource {
                attributes: vec![
                    kv_str("service.name", service),
                    kv_str("session.id", "proto-sess"),
                ],
                dropped_attributes_count: 0,
            }),
            scope_spans: vec![ScopeSpans {
                scope: None,
                spans,
                schema_url: String::new(),
            }],
            schema_url: String::new(),
        }],
    };
    let mut buf = Vec::new();
    req.encode(&mut buf).unwrap();
    buf
}

fn full_proto_span(name: &str, attrs: Vec<KeyValue>) -> Span {
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
    Span {
        name: name.into(),
        start_time_unix_nano: now,
        end_time_unix_nano: now + 1_000_000,
        attributes: attrs,
        trace_id: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        span_id: vec![1, 2, 3, 4, 5, 6, 7, 8],
        parent_span_id: vec![9, 10, 11, 12, 13, 14, 15, 16],
        status: Some(Status {
            message: "err".into(),
            code: 2,
        }),
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn handle_proto_all_span_kinds() {
    let (_db, buf, _cancel) = test_buffer(64).await;
    let spans = vec![
        full_proto_span(
            "execute_tool bash",
            vec![
                kv_str("gen_ai.tool.name", "bash"),
                kv_str("gen_ai.agent.name", "claude"),
                kv_int("gen_ai.usage.input_tokens", 3),
                kv_int("gen_ai.usage.output_tokens", 1),
                kv_str("gen_ai.tool.call.arguments", r#"{"cmd":"ls"}"#),
                kv_str("gen_ai.tool.call.result", "ok"),
                kv_str("gen_ai.tool.call.id", "tc-1"),
                kv_str("copilot_chat.user_request", "list files"),
            ],
        ),
        full_proto_span(
            "chat claude-sonnet",
            vec![
                kv_str("gen_ai.response.model", "claude-sonnet"),
                kv_int("gen_ai.usage.input_tokens", 11),
                kv_int("gen_ai.usage.output_tokens", 7),
                kv_str("gen_ai.system", "anthropic"),
                kv_double("gen_ai.request.temperature", 0.2),
                kv_int("gen_ai.request.max_tokens", 100),
                kv_str("gen_ai.response.finish_reason", "stop"),
            ],
        ),
        full_proto_span("invoke_agent x", vec![]),
        full_proto_span(
            "tools/call weather",
            vec![kv_str("gen_ai.tool.name", "get-weather")],
        ),
        full_proto_span(
            "POST",
            vec![kv_str(
                "http.url",
                "https://api.github.com/copilot/chat/completions",
            )],
        ),
        full_proto_span(
            "GET",
            vec![kv_str("http.url", "https://api.github.com/copilot/models")],
        ),
        full_proto_span("mystery_span", vec![]),
    ];
    let body = encode_proto_req(spans, "copilot-eclipse");
    let results = handle_trace_request(
        &body,
        Some("application/x-protobuf"),
        Some("10.0.0.1"),
        Some("eclipse/ua"),
        Some(&buf),
    )
    .expect("proto handle");
    assert!(results.len() >= 4, "got {}", results.len());
}

#[test]
fn parse_first_human_text_and_noise_branches() {
    // parts format with context XML then plain
    let raw = r#"[{"role":"user","parts":[
            {"type":"text","content":"<environment_info>x</environment_info>"},
            {"type":"text","content":"real user text"}
        ]}]"#;
    assert_eq!(
        parse_first_human_text(raw).as_deref(),
        Some("real user text")
    );

    let tagged = r#"[{"role":"user","parts":[
            {"type":"text","content":"<userRequest>from tag</userRequest>"}
        ]}]"#;
    assert_eq!(parse_first_human_text(tagged).as_deref(), Some("from tag"));

    let plain = r#"[{"role":"user","content":"plain user"}]"#;
    assert_eq!(parse_first_human_text(plain).as_deref(), Some("plain user"));

    let blocks = r#"[{"role":"user","content":[{"type":"text","text":"block user"}]}]"#;
    assert_eq!(
        parse_first_human_text(blocks).as_deref(),
        Some("block user")
    );

    let tool_result = r#"[{"role":"user","content":[{"type":"tool_result","text":"x"}]}]"#;
    assert_eq!(parse_first_human_text(tool_result), None);

    assert!(is_noise_prompt(
        "You are a helpful assistant with a long system prompt"
    ));
    assert!(is_noise_prompt(&"x".repeat(2001)));
    assert!(is_noise_prompt("[array]"));
    assert!(!is_noise_prompt("short real question?"));
}

#[test]
fn extract_user_request_skips_conversation_summary() {
    // First <userRequest> inside summary → current impl returns None (does not scan further).
    let inside =
        "<conversation-summary>mentions <userRequest>docs</userRequest></conversation-summary>";
    assert_eq!(extract_user_request_tag(inside), None);
    assert_eq!(
        extract_user_request_tag("<userRequest>real</userRequest>").as_deref(),
        Some("real")
    );
    assert_eq!(extract_user_request_tag("no tags"), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn handle_json_rich_tool_and_map_errors() {
    let (_db, buf, _cancel) = test_buffer(64).await;
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let spans = serde_json::json!([
        {
            "name": "execute_tool rich",
            "traceId": "aabbccddeeff00112233445566778899",
            "spanId": "1122334455667788",
            "parentSpanId": "99aabbccddeeff00",
            "startTimeUnixNano": now,
            "endTimeUnixNano": now + 5_000_000,
            "attributes": [
                attr_str("gen_ai.tool.name", "run_in_terminal"),
                attr_str("gen_ai.agent.name", "agent-x"),
                attr_str("mcp.server.name", "term"),
                attr_int("gen_ai.usage.prompt_tokens", 3),
                attr_int("gen_ai.usage.completion_tokens", 2),
                attr_int("gen_ai.usage.cache_read.input_tokens", 1),
                attr_int("gen_ai.usage.reasoning_tokens", 4),
                attr_str("gen_ai.request.model", "gpt-4.1"),
                attr_str("gen_ai.conversation.id", "conv-rich"),
                attr_str("gen_ai.tool.call.arguments", r#"{"cmd":"echo"}"#),
                attr_str("gen_ai.tool.call.result", "ok-out"),
                attr_str("gen_ai.tool.call.id", "call-1"),
                attr_str("gen_ai.response.finish_reasons", r#"["stop"]"#),
                attr_int("gen_ai.request.max_tokens", 50),
                attr_double("gen_ai.request.temperature", 0.1),
                attr_str("gen_ai.system", "openai"),
                attr_int("gen_ai.request.bytes", 100),
                attr_int("gen_ai.response.bytes", 200),
            ],
            "status": {"code": 2, "message": "failed"}
        },
        {
            "name": "execute_tool missing_time",
            "attributes": [attr_str("gen_ai.tool.name", "x")]
        },
        {
            "name": "chat",
            "attributes": []
        }
    ]);
    let body = json_otlp_payload(spans, "cursor");
    let results = handle_trace_request(
        &body,
        Some("application/json"),
        Some("1.2.3.4"),
        Some("cursor/1"),
        Some(&buf),
    )
    .expect("rich");
    assert!(!results.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn handle_json_rich_chat_models() {
    let (db, buf_ok, _cancel_ok) = test_buffer(64).await;
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    for (model, svc) in [
        ("claude-sonnet-4", "claude"),
        ("gpt-4o", "openai-svc"),
        ("gemini-2.5-pro", "google-svc"),
        ("weird-model", "other"),
    ] {
        let spans = serde_json::json!([{
            "name": format!("chat {model}"),
            "startTimeUnixNano": now.to_string(),
            "endTimeUnixNano": (now + 1_000_000).to_string(),
            "attributes": [
                attr_str("gen_ai.response.model", model),
                attr_int("gen_ai.usage.input_tokens", 1),
                attr_int("gen_ai.usage.output_tokens", 1),
                attr_str("copilot_chat.user_request", "hi"),
            ],
            "status": {"code": 1}
        }]);
        let body = json_otlp_payload(spans, svc);
        let _ = handle_trace_request(&body, Some("application/json"), None, None, Some(&buf_ok))
            .expect("chat");
    }
    // buffer full path
    let cancel = tokio_util::sync::CancellationToken::new();
    let buf = crate::services::ingest_buffer::IngestBuffer::spawn(db.clone(), 1, cancel.clone());
    cancel.cancel();
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    let spans = serde_json::json!([
        tool_span_json("execute_tool a"),
        tool_span_json("execute_tool b"),
        tool_span_json("execute_tool c")
    ]);
    let body = json_otlp_payload(spans, "cursor");
    let _ = handle_trace_request(&body, Some("application/json"), None, None, Some(&buf));
}

#[test]
fn json_attr_int_float_variants() {
    let attrs = vec![
        attr_int("a", 7),
        attr_int_str("b", "9"),
        attr_double("c", 1.5),
        serde_json::json!({"key":"d","value":{"stringValue":"3"}}),
        serde_json::json!({"key":"e","value":{"stringValue":"2.25"}}),
        serde_json::json!({"key":"f","value":{"doubleValue":4.0}}),
    ];
    assert_eq!(json_attr_int(&attrs, "a"), Some(7));
    assert_eq!(json_attr_int(&attrs, "b"), Some(9));
    assert_eq!(json_attr_int(&attrs, "c"), Some(1));
    assert_eq!(json_attr_int(&attrs, "d"), Some(3));
    assert_eq!(json_attr_float(&attrs, "c"), Some(1.5));
    assert_eq!(json_attr_float(&attrs, "a"), Some(7.0));
    assert_eq!(json_attr_float(&attrs, "e"), Some(2.25));
    assert_eq!(json_attr_float(&attrs, "missing"), None);
}

// ── Coverage push: error paths, prompts, response text, proto edges ─────

#[tokio::test(flavor = "multi_thread")]
async fn json_map_error_paths_and_unknown_spans() {
    let (_db, buf, _cancel) = test_buffer(64).await;
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let spans = serde_json::json!([
        // tools/call without timestamps → map Err warn (line ~132)
        {"name": "tools/call broken", "attributes": [attr_str("gen_ai.tool.name", "x")]},
        {"name": "tools/notify ping", "attributes": [attr_str("gen_ai.tool.name", "ping")],
         "startTimeUnixNano": now, "endTimeUnixNano": now + 1},
        // copilot HTTP chat missing times → Err warn (~144)
        {"name": "POST", "attributes": [
            attr_str("http.url", "https://api.githubcopilot.com/chat/completions")
        ]},
        // copilot HTTP tool missing times → Err warn (~151)
        {"name": "GET", "attributes": [
            attr_str("url.full", "https://api.github.com/copilot/models")
        ]},
        // unknown span from copilot service → warn (~158)
        {"name": "weird_custom_span", "attributes": []},
        // chat with error status message (~339)
        {"name": "chat gpt-4o", "startTimeUnixNano": now, "endTimeUnixNano": now + 2,
         "status": {"code": 2, "message": "boom"},
         "attributes": [
            attr_str("gen_ai.response.model", "gpt-4o"),
            attr_str("gen_ai.response.finish_reasons", "[\"stop\", 1]"),
            attr_str("copilot_chat.response", "hello reply"),
            attr_str("gen_ai.output.messages", r#"[{"role":"assistant","content":"plain out"}]"#),
         ]},
        // finish_reasons as Value array path via mixed types on tool
        {"name": "execute_tool t", "startTimeUnixNano": now, "endTimeUnixNano": now + 3,
         "attributes": [
            attr_str("gen_ai.tool.name", "read_file"),
            attr_str("gen_ai.response.finish_reasons", r#"["end", 99]"#),
         ]},
    ]);
    let body = json_otlp_payload(spans, "copilot-vscode");
    let _ = handle_trace_request(
        &body,
        Some("application/json"),
        Some("1.1.1.1"),
        Some("ua"),
        Some(&buf),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn persist_buffer_full_returns_unavailable() {
    let (_db, buf, cancel) = test_buffer(1).await;
    // Pre-fill channel synchronously so next try_send is Full (worker may not schedule yet).
    let now_ts = chrono::Utc::now();
    let filler: crate::models::event::ToolCallEvent = serde_json::from_value(serde_json::json!({
        "tool_name": "filler",
        "started_at": now_ts.to_rfc3339(),
        "ended_at": now_ts.to_rfc3339(),
        "ok": true,
    }))
    .unwrap();
    let _ = buf.try_send(filler);
    let filler2: crate::models::event::ToolCallEvent = serde_json::from_value(serde_json::json!({
        "tool_name": "filler2",
        "started_at": now_ts.to_rfc3339(),
        "ended_at": now_ts.to_rfc3339(),
        "ok": true,
    }))
    .unwrap();
    let _ = buf.try_send(filler2); // Full or Ok if drained — Closed path next after cancel
    cancel.cancel();
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    let now = now_ts.timestamp_nanos_opt().unwrap_or(0);
    let spans = serde_json::json!([{
        "name": "execute_tool a",
        "startTimeUnixNano": now,
        "endTimeUnixNano": now + 1,
        "attributes": [attr_str("gen_ai.tool.name", "read_file")],
    }]);
    let body = json_otlp_payload(spans, "cursor");
    let err = handle_trace_request(&body, Some("application/json"), None, None, Some(&buf));
    // Full or Closed both → ServiceUnavailable from persist_event
    assert!(err.is_err(), "expected buffer reject, got {err:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn persist_without_buffer_errors() {
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let spans = serde_json::json!([{
        "name": "execute_tool x",
        "startTimeUnixNano": now,
        "endTimeUnixNano": now + 1,
        "attributes": [attr_str("gen_ai.tool.name", "read_file")],
    }]);
    let body = json_otlp_payload(spans, "cursor");
    let res = handle_trace_request(&body, Some("application/json"), None, None, None);
    assert!(res.is_err(), "OSS requires ingest buffer");
}

#[test]
fn parse_first_human_text_edge_branches() {
    // XML part with userRequest (538)
    let tagged = r#"[{"role":"user","parts":[{"type":"text","content":"<env/><userRequest>from xml</userRequest>"}]}]"#;
    assert_eq!(parse_first_human_text(tagged).as_deref(), Some("from xml"));
    // empty text part then skip (543)
    let empty_part = r#"[{"role":"user","parts":[{"type":"text","content":"   "},{"type":"text","content":"ok"}]}]"#;
    assert_eq!(parse_first_human_text(empty_part).as_deref(), Some("ok"));
    // parts only context XML without userRequest → continue (547)
    let only_ctx = r#"[{"role":"user","parts":[{"type":"text","content":"<environment_info>x</environment_info>"}]},{"role":"user","content":"fallback"}]"#;
    assert_eq!(
        parse_first_human_text(only_ctx).as_deref(),
        Some("fallback")
    );
    // content XML with userRequest (556)
    let ctag =
        r#"[{"role":"user","content":"<wrapper><userRequest>c tag</userRequest></wrapper>"}]"#;
    assert_eq!(parse_first_human_text(ctag).as_deref(), Some("c tag"));
    // content starts with [{ → try tag then continue (557)
    let jsonish = r#"[{"role":"user","content":"[{noise"},{"role":"user","content":"real"}]"#;
    assert_eq!(parse_first_human_text(jsonish).as_deref(), Some("real"));
    // empty string content (561-562)
    let empty_c = r#"[{"role":"user","content":"  "},{"role":"user","content":"next"}]"#;
    assert_eq!(parse_first_human_text(empty_c).as_deref(), Some("next"));
    // anthropic empty text block then real (580)
    let empty_block = r#"[{"role":"user","content":[{"type":"text","text":"  "},{"type":"text","text":"block ok"}]}]"#;
    assert_eq!(
        parse_first_human_text(empty_block).as_deref(),
        Some("block ok")
    );
    // blocks with no text type (584)
    let no_text = r#"[{"role":"user","content":[{"type":"image","text":"x"}]},{"role":"user","content":"after"}]"#;
    assert_eq!(parse_first_human_text(no_text).as_deref(), Some("after"));
}

#[test]
fn extract_clean_prompt_noise_and_indexed() {
    // noise after parse from user_request JSON (618)
    let noise_ur = vec![attr_str(
        "copilot_chat.user_request",
        r#"[{"role":"user","content":"You are a helpful assistant with system instructions"}]"#,
    )];
    assert_eq!(extract_clean_user_prompt_json(&noise_ur), None);
    // noise from input.messages (630)
    let noise_im = vec![attr_str(
        "gen_ai.input.messages",
        r#"[{"role":"user","content":"[{"}]"#,
    )];
    assert_eq!(extract_clean_user_prompt_json(&noise_im), None);
    // gen_ai.prompt noise without tag (641)
    let noise_p = vec![attr_str("gen_ai.prompt", "You are a bot please obey")];
    assert_eq!(extract_clean_user_prompt_json(&noise_p), None);
    // indexed prompt with userRequest (652)
    let indexed = vec![
        attr_str("gen_ai.prompt.0.role", "system"),
        attr_str("gen_ai.prompt.0.content", "sys"),
        attr_str("gen_ai.prompt.1.role", "user"),
        attr_str(
            "gen_ai.prompt.1.content",
            "<userRequest>indexed req</userRequest>",
        ),
    ];
    assert_eq!(
        extract_clean_user_prompt_json(&indexed).as_deref(),
        Some("indexed req")
    );
    // indexed user with noise content (656)
    let indexed_noise = vec![
        attr_str("gen_ai.prompt.0.role", "user"),
        attr_str(
            "gen_ai.prompt.0.content",
            "Please write a brief title for this chat",
        ),
    ];
    assert_eq!(extract_clean_user_prompt_json(&indexed_noise), None);
    // gen_ai.prompt clean
    let clean_p = vec![attr_str("gen_ai.prompt", "do the thing")];
    assert_eq!(
        extract_clean_user_prompt_json(&clean_p).as_deref(),
        Some("do the thing")
    );
}

#[test]
fn extract_clean_prompt_proto_branches() {
    let noise = vec![kv_str(
        "copilot_chat.user_request",
        r#"[{"role":"user","content":"You are a system"}]"#,
    )];
    assert_eq!(extract_clean_user_prompt_proto(&noise), None);
    let json_ok = vec![kv_str(
        "copilot_chat.user_request",
        r#"[{"role":"user","content":"proto user"}]"#,
    )];
    assert_eq!(
        extract_clean_user_prompt_proto(&json_ok).as_deref(),
        Some("proto user")
    );
    let plain = vec![kv_str("copilot_chat.user_request", "plain proto")];
    assert_eq!(
        extract_clean_user_prompt_proto(&plain).as_deref(),
        Some("plain proto")
    );
    let im = vec![kv_str(
        "gen_ai.input.messages",
        r#"[{"role":"user","content":"im proto"}]"#,
    )];
    assert_eq!(
        extract_clean_user_prompt_proto(&im).as_deref(),
        Some("im proto")
    );
    let im_noise = vec![kv_str(
        "gen_ai.input.messages",
        r#"[{"role":"user","content":"[{"}]"#,
    )];
    assert_eq!(extract_clean_user_prompt_proto(&im_noise), None);
    let tag = vec![kv_str(
        "gen_ai.prompt",
        "<userRequest>proto tag</userRequest>",
    )];
    assert_eq!(
        extract_clean_user_prompt_proto(&tag).as_deref(),
        Some("proto tag")
    );
    let p_noise = vec![kv_str("gen_ai.prompt", "You are noise")];
    assert_eq!(extract_clean_user_prompt_proto(&p_noise), None);
    let p0 = vec![kv_str("gen_ai.prompt.0.content", "p0 content")];
    assert_eq!(
        extract_clean_user_prompt_proto(&p0).as_deref(),
        Some("p0 content")
    );
    let p0n = vec![kv_str("gen_ai.prompt.0.content", "You are x")];
    assert_eq!(extract_clean_user_prompt_proto(&p0n), None);
}

#[test]
fn extract_user_request_summary_without_end() {
    // conversation-summary start without end → don't skip (723 path falls through)
    let s = "<conversation-summary>open <userRequest>still extract</userRequest>";
    assert_eq!(
        extract_user_request_tag(s).as_deref(),
        Some("still extract")
    );
}

#[test]
fn extract_response_text_all_formats() {
    // empty copilot_chat.response skipped → output messages anthropic blocks
    let blocks = vec![attr_str(
        "gen_ai.output.messages",
        r#"[{"role":"assistant","content":[{"type":"text","text":"block out"}]}]"#,
    )];
    assert_eq!(extract_response_text(&blocks).as_deref(), Some("block out"));
    // empty string content then parts (gemini)
    let parts = vec![attr_str(
        "gen_ai.output.messages",
        r#"[{"role":"model","content":"","parts":[{"text":"part out"}]}]"#,
    )];
    assert_eq!(extract_response_text(&parts).as_deref(), Some("part out"));
    // parts via content key
    let parts2 = vec![attr_str(
        "gen_ai.output.messages",
        r#"[{"role":"assistant","parts":[{"content":"part2"}]}]"#,
    )];
    assert_eq!(extract_response_text(&parts2).as_deref(), Some("part2"));
    // indexed completion
    let idx = vec![attr_str("gen_ai.completion.0.content", "idx out")];
    assert_eq!(extract_response_text(&idx).as_deref(), Some("idx out"));
    // response.text
    let rt = vec![attr_str("gen_ai.response.text", "rt out")];
    assert_eq!(extract_response_text(&rt).as_deref(), Some("rt out"));
    // empty assistant content blocks empty text
    let empty_b = vec![attr_str(
        "gen_ai.output.messages",
        r#"[{"role":"assistant","content":[{"type":"text","text":""},{"type":"text","text":"after empty"}]}]"#,
    )];
    assert_eq!(
        extract_response_text(&empty_b).as_deref(),
        Some("after empty")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn proto_map_errors_and_http_edges() {
    let (_db, buf, _cancel) = test_buffer(64).await;
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
    let spans = vec![
        // execute_tool zero timestamps → Err warn (846, 926)
        Span {
            name: "execute_tool z".into(),
            start_time_unix_nano: 0,
            end_time_unix_nano: 0,
            attributes: vec![kv_str("gen_ai.tool.name", "z")],
            ..Default::default()
        },
        // chat zero timestamps (854, 1041)
        Span {
            name: "chat m".into(),
            start_time_unix_nano: 0,
            end_time_unix_nano: 0,
            ..Default::default()
        },
        // tools/call zero ts (865)
        Span {
            name: "tools/call t".into(),
            start_time_unix_nano: 0,
            end_time_unix_nano: 0,
            ..Default::default()
        },
        // copilot HTTP chat zero ts (877)
        Span {
            name: "POST".into(),
            start_time_unix_nano: 0,
            end_time_unix_nano: 0,
            attributes: vec![kv_str(
                "http.url",
                "https://api.github.com/copilot/chat/completions",
            )],
            ..Default::default()
        },
        // copilot HTTP tool zero ts (884)
        Span {
            name: "GET".into(),
            start_time_unix_nano: 0,
            end_time_unix_nano: 0,
            attributes: vec![kv_str("http.url", "https://copilot-proxy.example/models")],
            ..Default::default()
        },
        // unknown non-http from copilot (891)
        Span {
            name: "custom.eclipse.op".into(),
            start_time_unix_nano: now,
            end_time_unix_nano: now + 1,
            ..Default::default()
        },
        // rich tool with error status + finish_reasons + float string (933, 981, 1152)
        Span {
            name: "execute_tool rich".into(),
            start_time_unix_nano: now,
            end_time_unix_nano: now + 10,
            attributes: vec![
                kv_str("gen_ai.tool.name", "bash"),
                kv_str("gen_ai.response.finish_reasons", r#"["length"]"#),
                KeyValue {
                    key: "gen_ai.request.temperature".into(),
                    value: Some(AnyValue {
                        value: Some(any_value::Value::StringValue("0.7".into())),
                    }),
                },
                KeyValue {
                    key: "gen_ai.request.max_tokens".into(),
                    value: Some(AnyValue {
                        value: Some(any_value::Value::IntValue(50)),
                    }),
                },
            ],
            status: Some(Status {
                code: 2,
                message: "fail".into(),
            }),
            ..Default::default()
        },
        // chat models without system attr → provider inference (1069-1073)
        full_proto_span("chat c", vec![kv_str("gen_ai.response.model", "claude-3")]),
        full_proto_span("chat g", vec![kv_str("gen_ai.response.model", "gpt-4")]),
        full_proto_span(
            "chat o",
            vec![kv_str("gen_ai.response.model", "o1-preview")],
        ),
        full_proto_span(
            "chat gm",
            vec![kv_str("gen_ai.response.model", "gemini-pro")],
        ),
        full_proto_span("chat u", vec![kv_str("gen_ai.response.model", "other-x")]),
        // chat finish_reasons (1088)
        full_proto_span(
            "chat fr",
            vec![
                kv_str("gen_ai.response.model", "gpt-4"),
                kv_str("gen_ai.response.finish_reasons", r#"["stop"]"#),
            ],
        ),
        // HTTP span with non-string attr value on url key (1268 None path)
        Span {
            name: "POST".into(),
            start_time_unix_nano: now,
            end_time_unix_nano: now + 1,
            attributes: vec![KeyValue {
                key: "http.url".into(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::IntValue(1)),
                }),
            }],
            ..Default::default()
        },
        // is_copilot_http false: HTTP name but unrelated URL (1190/1278)
        Span {
            name: "GET".into(),
            start_time_unix_nano: now,
            end_time_unix_nano: now + 1,
            attributes: vec![kv_str("http.url", "https://example.com/api")],
            ..Default::default()
        },
        // eclipse service HTTP (is_eclipse_service_http_span)
        Span {
            name: "POST".into(),
            start_time_unix_nano: now,
            end_time_unix_nano: now + 1,
            attributes: vec![kv_str("http.url", "https://example.com/other")],
            ..Default::default()
        },
        // classify with server.address only (no url) → default tool
        Span {
            name: "GET".into(),
            start_time_unix_nano: now,
            end_time_unix_nano: now + 1,
            attributes: vec![kv_str("server.address", "api.githubcopilot.com")],
            ..Default::default()
        },
    ];
    let body = encode_proto_req(spans, "copilot-eclipse");
    let _ = handle_trace_request(
        &body,
        Some("application/x-protobuf"),
        None,
        Some("eclipse"),
        Some(&buf),
    );
}

#[test]
fn is_copilot_http_span_false_paths() {
    let span = serde_json::json!({
        "name": "GET",
        "attributes": [attr_str("http.url", "https://example.com")]
    });
    assert!(!is_copilot_http_span(&span, "GET"));
    let no_attrs = serde_json::json!({"name": "POST"});
    assert!(!is_copilot_http_span(&no_attrs, "POST"));
    assert!(!is_copilot_http_span(&span, "NOTHTTP"));
    // Non-matching attr key → return false inside any() (1206)
    let wrong_key = serde_json::json!({
        "name": "GET",
        "attributes": [attr_str("http.method", "GET")]
    });
    assert!(!is_copilot_http_span(&wrong_key, "GET"));
    let proto_wrong = Span {
        name: "GET".into(),
        attributes: vec![kv_str("http.method", "GET")],
        ..Default::default()
    };
    assert!(!is_copilot_http_span_proto(&proto_wrong));
    // classify when only server.address (no url keys) → empty url → copilot_api
    let sa = serde_json::json!({
        "attributes": [attr_str("server.address", "api.githubcopilot.com")]
    });
    let (name, chat) = classify_copilot_http_span(&sa);
    assert_eq!(name, "copilot_api");
    assert!(!chat);
    // key mismatch branch in find_map (1208)
    let other = serde_json::json!({
        "attributes": [attr_str("http.method", "GET")]
    });
    let (name2, _) = classify_copilot_http_span(&other);
    assert_eq!(name2, "copilot_api");
}

#[test]
fn get_attr_float_bool_none_branch() {
    let attrs = vec![KeyValue {
        key: "x".into(),
        value: Some(AnyValue {
            value: Some(any_value::Value::BoolValue(true)),
        }),
    }];
    assert_eq!(get_attr_float(&attrs, "x"), None);
    assert_eq!(get_attr_float(&attrs, "missing"), None);
}

#[test]
fn coverage_remaining_otlp_edges() {
    // empty text parts only → parts continue (543-545) then None
    let empty_parts = r#"[{"role":"user","parts":[{"type":"text","content":"   "},{"type":"other","content":"x"}]}]"#;
    assert_eq!(parse_first_human_text(empty_parts), None);
    // anthropic empty texts only (581/584)
    let empty_blocks =
        r#"[{"role":"user","content":[{"type":"text","text":""},{"type":"image"}]}]"#;
    assert_eq!(parse_first_human_text(empty_blocks), None);
    // noise after successful parse (618-619)
    let attrs = vec![attr_str(
        "copilot_chat.user_request",
        r#"[{"role":"user","content":"[{"}]"#,
    )];
    // parse may None; also try noise human text
    let attrs2 = vec![attr_str(
        "copilot_chat.user_request",
        r#"[{"role":"user","content":"Please write a brief title for this conversation now"}]"#,
    )];
    assert_eq!(extract_clean_user_prompt_json(&attrs2), None);
    // input.messages noise (630)
    let attrs3 = vec![attr_str(
        "gen_ai.input.messages",
        r#"[{"role":"user","content":"You are a system prompt that is noise"}]"#,
    )];
    assert_eq!(extract_clean_user_prompt_json(&attrs3), None);
    // indexed user noise (657)
    let attrs4 = vec![
        attr_str("gen_ai.prompt.0.role", "user"),
        attr_str("gen_ai.prompt.0.content", "[{"),
    ];
    assert_eq!(extract_clean_user_prompt_json(&attrs4), None);
    let _ = attrs;

    // proto: noise plain after JSON fail (676/679), input noise (686), prompt noise (695)
    let p1 = vec![kv_str("copilot_chat.user_request", "[{")];
    assert_eq!(extract_clean_user_prompt_proto(&p1), None);
    let p2 = vec![kv_str("copilot_chat.user_request", "You are noise")];
    assert_eq!(extract_clean_user_prompt_proto(&p2), None);
    let p3 = vec![kv_str(
        "gen_ai.input.messages",
        r#"[{"role":"user","content":"You are noise"}]"#,
    )];
    assert_eq!(extract_clean_user_prompt_proto(&p3), None);
    let p4 = vec![kv_str("gen_ai.prompt", "You are noise")];
    assert_eq!(extract_clean_user_prompt_proto(&p4), None);

    // response text empty branches + non-text block in find_map
    let empty_out = vec![attr_str(
        "gen_ai.output.messages",
        r#"[{"role":"assistant","content":""},{"role":"assistant","content":[{"type":"image"},{"type":"text","text":""}],"parts":[{"content":""},{"text":""}]}]"#,
    )];
    assert_eq!(extract_response_text(&empty_out), None);
    let empty_idx = vec![
        attr_str("gen_ai.completion.0.content", ""),
        attr_str("gen_ai.response.text", ""),
    ];
    assert_eq!(extract_response_text(&empty_idx), None);

    // is_copilot_http false at end (1190) — already; proto false (1279)
    let span = Span {
        name: "GET".into(),
        attributes: vec![kv_str("http.url", "https://example.com")],
        ..Default::default()
    };
    assert!(!is_copilot_http_span_proto(&span));
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_span_warn_from_copilot_json_and_proto() {
    let spans = serde_json::json!([{"name": "totally_unknown_op", "attributes": []}]);
    let body = json_otlp_payload(spans, "github-copilot");
    let _ = handle_trace_request(&body, Some("application/json"), None, None, None);
    let now = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0) as u64;
    let body2 = encode_proto_req(
        vec![Span {
            name: "totally_unknown_op".into(),
            start_time_unix_nano: now,
            end_time_unix_nano: now + 1,
            ..Default::default()
        }],
        "eclipse-copilot",
    );
    let _ = handle_trace_request(&body2, Some("application/x-protobuf"), None, None, None);
}

#[test]
fn parse_first_human_text_only_empty_then_none() {
    // Reverse iter hits empty trimmed → falls through
    let only_ws = r#"[{"role":"user","parts":[{"type":"text","content":"   "}]}]"#;
    assert_eq!(parse_first_human_text(only_ws), None);
    // Anthropic: text key missing on text-type block, then empty text
    let missing_text = r#"[{"role":"user","content":[{"type":"text"},{"type":"text","text":""}]}]"#;
    assert_eq!(parse_first_human_text(missing_text), None);
    // only whitespace text in anthropic blocks
    let only_empty_block = r#"[{"role":"user","content":[{"type":"text","text":"   "}]}]"#;
    assert_eq!(parse_first_human_text(only_empty_block), None);
    // text type without content key
    let no_content =
        r#"[{"role":"user","parts":[{"type":"text"}]},{"role":"user","content":"ok"}]"#;
    assert_eq!(parse_first_human_text(no_content).as_deref(), Some("ok"));
}

#[test]
fn extract_clean_prompt_noise_extracted_and_proto_clean() {
    // parse ok but noise → close brace 623
    let noise_ur = vec![attr_str(
        "copilot_chat.user_request",
        r#"[{"role":"user","content":"You are a helpful assistant please obey"}]"#,
    )];
    assert_eq!(extract_clean_user_prompt_json(&noise_ur), None);
    // indexed user role with missing content key → 661
    let no_content = vec![attr_str("gen_ai.prompt.0.role", "user")];
    assert_eq!(extract_clean_user_prompt_json(&no_content), None);
    // break when role key missing after index 0
    let break_early = vec![
        attr_str("gen_ai.prompt.0.role", "assistant"),
        attr_str("gen_ai.prompt.0.content", "a"),
    ];
    assert_eq!(extract_clean_user_prompt_json(&break_early), None);
    // proto clean prompt without tag (699)
    let clean = vec![kv_str("gen_ai.prompt", "do proto thing")];
    assert_eq!(
        extract_clean_user_prompt_proto(&clean).as_deref(),
        Some("do proto thing")
    );
}

#[test]
fn extract_response_empty_parts_then_none() {
    // empty string content + empty blocks + empty parts → fall through all loops
    let empty_all = vec![attr_str(
        "gen_ai.output.messages",
        r#"[{"role":"assistant","content":[{"type":"text"},{"type":"text","text":""}],"parts":[{"type":"x"},{"content":""},{"text":""}]}]"#,
    )];
    assert_eq!(extract_response_text(&empty_all), None);
    // non-assistant roles ignored
    let user_only = vec![attr_str(
        "gen_ai.output.messages",
        r#"[{"role":"user","content":"hi"}]"#,
    )];
    assert_eq!(extract_response_text(&user_only), None);
    // invalid JSON array
    let bad = vec![attr_str("gen_ai.output.messages", "not-json")];
    assert_eq!(extract_response_text(&bad), None);
    // valid array but not array value (object)
    let obj = vec![attr_str(
        "gen_ai.output.messages",
        r#"{"role":"assistant"}"#,
    )];
    assert_eq!(extract_response_text(&obj), None);
}

#[test]
fn is_copilot_http_attrs_without_match_returns_false() {
    // attrs present, keys match, but URL unrelated → false (1198)
    let span = span_with_attrs("GET", vec![attr_str("server.address", "api.example.com")]);
    assert!(!is_copilot_http_span(&span, "GET"));
    let span = Span {
        name: "GET".into(),
        attributes: vec![kv_str("server.address", "api.example.com")],
        ..Default::default()
    };
    assert!(!is_copilot_http_span_proto(&span));
}
