use prost::Message;
use sqlx::PgPool;
use tracing::{info, warn};

use crate::errors::AppError;
use crate::models::event::ToolCallEvent;
use crate::services::event_service;

/// Dispatcher: selects protobuf or JSON parser based on Content-Type.
pub fn handle_trace_request(
    body: &[u8],
    content_type: Option<&str>,
    client_ip: Option<&str>,
    user_agent: Option<&str>,
    pool: &PgPool,
) -> Result<Vec<serde_json::Value>, AppError> {
    if content_type.map(|ct| ct.contains("application/json")).unwrap_or(false) {
        return handle_trace_request_json(body, client_ip, user_agent, pool);
    }
    handle_trace_request_proto(body, client_ip, user_agent, pool)
}

/// Parse OTLP JSON (format sent by VS Code / OpenTelemetry JS SDK).
fn handle_trace_request_json(body: &[u8], client_ip: Option<&str>, user_agent: Option<&str>, pool: &PgPool) -> Result<Vec<serde_json::Value>, AppError> {
    let root: serde_json::Value = serde_json::from_slice(body)
        .map_err(|e| AppError::Validation(format!("failed to decode OTLP JSON: {e}")))?;

    let mut results = Vec::new();

    let resource_spans = root.get("resourceSpans").and_then(|v| v.as_array());
    let Some(resource_spans) = resource_spans else {
        return Ok(results);
    };

    for rs in resource_spans {
        let resource_attrs = rs.pointer("/resource/attributes")
            .and_then(|v| v.as_array())
            .map(|a| a.as_slice())
            .unwrap_or(&[]);

        let service_name = json_attr_str(resource_attrs, "service.name");
        let session_id = json_attr_str(resource_attrs, "session.id");
        // Infer IDE from user_agent if not available from resource
        let ide = infer_ide(user_agent, service_name.as_deref());

        let scope_spans = rs.get("scopeSpans").and_then(|v| v.as_array());
        let Some(scope_spans) = scope_spans else { continue };

        for ss in scope_spans {
            let spans = ss.get("spans").and_then(|v| v.as_array());
            let Some(spans) = spans else { continue };

            for span in spans {
                let span_name = span.get("name").and_then(|v| v.as_str()).unwrap_or("");

                // One-time debug: log all attribute keys for diagnosis (gated by env var)
                if std::env::var("AGENT_METER_DEBUG_SPANS").is_ok() {
                    let attrs_dbg = span.get("attributes").and_then(|v| v.as_array());
                    let attr_keys: Vec<&str> = attrs_dbg.map(|a| a.iter()
                            .filter_map(|kv| kv.get("key").and_then(|k| k.as_str()))
                            .collect())
                        .unwrap_or_default();
                    info!("span '{}' attribute keys: {:?} | resource service={:?} session={:?}",
                        span_name, attr_keys, service_name, session_id);
                    // If this is a chat span, dump the raw user_request value
                    if span_name.starts_with("chat") {
                        let raw_user_req = span.get("attributes").and_then(|v| v.as_array())
                            .and_then(|a| a.iter().find(|kv| kv.get("key").and_then(|k| k.as_str()) == Some("copilot_chat.user_request")))
                            .map(|kv| kv.to_string());
                        info!("chat span user_request raw: {:?}", raw_user_req);
                    }
                }

                // VS Code sends: "execute_tool <tool_name>", "chat <model>", "invoke_agent <agent>"
                if span_name.starts_with("execute_tool") {
                    let tool_hint = span_name.strip_prefix("execute_tool").map(|s| s.trim()).filter(|s| !s.is_empty());
                    match map_tool_call_json(span, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, tool_hint) {
                        Ok(event) => {
                            match tokio::task::block_in_place(|| {
                                tokio::runtime::Handle::current().block_on(async {
                                    event_service::insert_tool_call(pool, event).await
                                })
                            }) {
                                Ok(record) => {
                                    results.push(serde_json::json!({
                                        "event_id": record.event_id,
                                        "tool_name": record.tool_name,
                                        "duration_ms": record.duration_ms,
                                    }));
                                }
                                Err(e) => warn!("failed to insert JSON OTLP tool_call: {e}"),
                            }
                        }
                        Err(e) => warn!("failed to map JSON OTLP execute_tool span: {e}"),
                    }
                } else if span_name.starts_with("chat") {
                    // "chat <model>" spans: LLM completion with token accounting
                    let model_hint = span_name.strip_prefix("chat").map(|s| s.trim()).filter(|s| !s.is_empty());
                    match map_chat_span_json(span, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, model_hint) {
                        Ok(event) => {
                            match tokio::task::block_in_place(|| {
                                tokio::runtime::Handle::current().block_on(async {
                                    event_service::insert_tool_call(pool, event).await
                                })
                            }) {
                                Ok(record) => {
                                    results.push(serde_json::json!({
                                        "event_id": record.event_id,
                                        "tool_name": record.tool_name,
                                        "duration_ms": record.duration_ms,
                                    }));
                                }
                                Err(e) => warn!("failed to insert JSON OTLP chat span: {e}"),
                            }
                        }
                        Err(e) => warn!("failed to map JSON OTLP chat span: {e}"),
                    }
                } else if span_name.starts_with("invoke_agent") {
                    // "invoke_agent <agent_name>" — agent session boundary, ignore for now
                } else {
                    warn!("unknown OTLP span name (json): {}", span_name);
                }
            }
        }
    }

    Ok(results)
}

fn map_tool_call_json(
    span: &serde_json::Value,
    service_name: Option<&str>,
    session_id: Option<&str>,
    ide: Option<&str>,
    client_ip: Option<&str>,
    user_agent: Option<&str>,
    tool_name_hint: Option<&str>,
) -> Result<ToolCallEvent, &'static str> {
    let attrs = span.get("attributes").and_then(|v| v.as_array());
    let attrs_slice: &[serde_json::Value] = attrs.map(|a| a.as_slice()).unwrap_or(&[]);

    // Tool name: attribute → span name suffix → fallback to raw span name
    let tool_name = json_attr_str(attrs_slice, "gen_ai.tool.name")
        .or_else(|| tool_name_hint.map(|s| s.to_string()))
        .or_else(|| span.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .ok_or("missing tool name")?;

    let agent_name = json_attr_str(attrs_slice, "gen_ai.agent.name")
        .or_else(|| service_name.map(|s| s.to_string()));

    let mcp_server = json_attr_str(attrs_slice, "mcp.server.name")
        .or_else(|| json_attr_str(attrs_slice, "mcp.server"));

    let start_ns: i64 = span.get("startTimeUnixNano")
        .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64()))
        .ok_or("missing startTimeUnixNano")?;
    let end_ns: i64 = span.get("endTimeUnixNano")
        .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64()))
        .ok_or("missing endTimeUnixNano")?;

    let start = chrono::DateTime::from_timestamp_nanos(start_ns);
    let end = chrono::DateTime::from_timestamp_nanos(end_ns);

    let status_code = span.pointer("/status/code").and_then(|v| v.as_i64()).unwrap_or(0);
    let ok = status_code != 2;
    let error = if !ok {
        span.pointer("/status/message").and_then(|v| v.as_str()).map(|s| s.to_string())
    } else {
        None
    };

    let input_tokens = json_attr_int(attrs_slice, "gen_ai.usage.input_tokens")
        .or_else(|| json_attr_int(attrs_slice, "gen_ai.usage.prompt_tokens"));
    let output_tokens = json_attr_int(attrs_slice, "gen_ai.usage.output_tokens")
        .or_else(|| json_attr_int(attrs_slice, "gen_ai.usage.completion_tokens"));
    // VS Code Copilot uses dot notation: cache_read.input_tokens (not underscore)
    let cached_tokens = json_attr_int(attrs_slice, "gen_ai.usage.cache_read.input_tokens")
        .or_else(|| json_attr_int(attrs_slice, "gen_ai.usage.cache_read_input_tokens"))
        .or_else(|| json_attr_int(attrs_slice, "gen_ai.usage.cached_tokens"))
        .or_else(|| json_attr_int(attrs_slice, "gen_ai.usage.input_tokens_cached"));
    let model = json_attr_str(attrs_slice, "gen_ai.response.model")
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.request.model"));
    // Conversation/thread ID from multiple possible attributes + traceId fallback
    let trace_id = span.get("traceId").and_then(|v| v.as_str()).map(|s| s.to_string());
    let conversation_id = json_attr_str(attrs_slice, "gen_ai.conversation.id")
        .or_else(|| json_attr_str(attrs_slice, "copilot_chat.chat_session_id"))
        .or_else(|| json_attr_str(attrs_slice, "copilot.conversation.id"))
        .or_else(|| json_attr_str(attrs_slice, "thread.id"))
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.thread.id"))
        .or_else(|| session_id.map(|s| s.to_string()))
        .or(trace_id);
    let response_bytes = json_attr_int(attrs_slice, "gen_ai.response.bytes");
    let request_bytes = json_attr_int(attrs_slice, "gen_ai.request.bytes");

    // User prompt: VS Code Copilot sends it as copilot_chat.user_request;
    // fall back to GenAI semantic-convention keys if other IDEs are used.
    // Filter out tool_result payloads (Anthropic agentic loop sends "[{\"type\":\"tool_result\"...}]"
    // as the "user turn" for subsequent LLM calls — we only want the actual human text).
    let user_prompt = json_attr_str(attrs_slice, "copilot_chat.user_request")
        .filter(|s| !s.trim_start().starts_with("[{"))
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.prompt"))
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.prompt.0.content"))
        .or_else(|| json_user_message_content(attrs_slice))
        .or_else(|| extract_first_human_text(attrs_slice));

    let mut metadata = serde_json::json!({});
    if let Some(svc) = service_name { metadata["service_name"] = serde_json::json!(svc); }
    if let Some(sid) = session_id { metadata["session_id"] = serde_json::json!(sid); }
    if let Some(a) = &agent_name { metadata["agent"] = serde_json::json!(a); }

    Ok(ToolCallEvent {
        event_id: uuid::Uuid::new_v4(),
        task_id: None,
        repo: None,
        branch: None,
        ide: ide.map(|s| s.to_string()).or_else(|| Some("copilot-vscode".into())),
        agent: agent_name,
        skill: None,
        mcp_server,
        tool_name,
        started_at: start,
        ended_at: end,
        ok,
        error,
        request_bytes: request_bytes.map(|b| b as i32),
        response_bytes: response_bytes.map(|b| b as i32),
        estimated_input_tokens: input_tokens.map(|t| t as i32),
        estimated_output_tokens: output_tokens.map(|t| t as i32),
        request_sha256: None,
        response_sha256: None,
        metadata: Some(metadata),
        model,
        cached_tokens: cached_tokens.map(|t| t as i32),
        conversation_id,
        client_ip: client_ip.map(|s| s.to_string()),
        user_agent: user_agent.map(|s| s.to_string()),
        user_prompt,
    })
}

/// Record a "chat <model>" LLM completion span for token accounting.
fn map_chat_span_json(
    span: &serde_json::Value,
    service_name: Option<&str>,
    session_id: Option<&str>,
    ide: Option<&str>,
    client_ip: Option<&str>,
    user_agent: Option<&str>,
    model_hint: Option<&str>,
) -> Result<ToolCallEvent, &'static str> {
    let attrs = span.get("attributes").and_then(|v| v.as_array());
    let attrs_slice: &[serde_json::Value] = attrs.map(|a| a.as_slice()).unwrap_or(&[]);

    let start_ns: i64 = span.get("startTimeUnixNano")
        .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64()))
        .ok_or("missing startTimeUnixNano")?;
    let end_ns: i64 = span.get("endTimeUnixNano")
        .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()).or_else(|| v.as_i64()))
        .ok_or("missing endTimeUnixNano")?;

    let start = chrono::DateTime::from_timestamp_nanos(start_ns);
    let end = chrono::DateTime::from_timestamp_nanos(end_ns);

    let status_code = span.pointer("/status/code").and_then(|v| v.as_i64()).unwrap_or(0);
    let ok = status_code != 2;
    let error = if !ok {
        span.pointer("/status/message").and_then(|v| v.as_str()).map(|s| s.to_string())
    } else {
        None
    };

    let input_tokens = json_attr_int(attrs_slice, "gen_ai.usage.input_tokens")
        .or_else(|| json_attr_int(attrs_slice, "gen_ai.usage.prompt_tokens"));
    let output_tokens = json_attr_int(attrs_slice, "gen_ai.usage.output_tokens")
        .or_else(|| json_attr_int(attrs_slice, "gen_ai.usage.completion_tokens"));
    let cached_tokens = json_attr_int(attrs_slice, "gen_ai.usage.cache_read.input_tokens")
        .or_else(|| json_attr_int(attrs_slice, "gen_ai.usage.cache_read_input_tokens"))
        .or_else(|| json_attr_int(attrs_slice, "gen_ai.usage.cached_tokens"))
        .or_else(|| json_attr_int(attrs_slice, "gen_ai.usage.input_tokens_cached"));
    let model = json_attr_str(attrs_slice, "gen_ai.response.model")
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.request.model"))
        .or_else(|| model_hint.map(|s| s.to_string()));
    let system = json_attr_str(attrs_slice, "gen_ai.system");
    let trace_id = span.get("traceId").and_then(|v| v.as_str()).map(|s| s.to_string());
    let conversation_id = json_attr_str(attrs_slice, "gen_ai.conversation.id")
        .or_else(|| json_attr_str(attrs_slice, "copilot_chat.chat_session_id"))
        .or_else(|| json_attr_str(attrs_slice, "copilot.conversation.id"))
        .or_else(|| json_attr_str(attrs_slice, "thread.id"))
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.thread.id"))
        .or_else(|| session_id.map(|s| s.to_string()))
        .or(trace_id);

    // Use gen_ai.system as mcp_server equivalent for LLM provider grouping
    let mcp_server = system.or_else(|| {
        model.as_ref().map(|m| {
            if m.contains("claude") { "anthropic".to_string() }
            else if m.contains("gpt") || m.contains("o1") || m.contains("o3") { "openai".to_string() }
            else if m.contains("gemini") { "google".to_string() }
            else { "unknown".to_string() }
        })
    });

    let agent_name = json_attr_str(attrs_slice, "gen_ai.agent.name")
        .or_else(|| service_name.map(|s| s.to_string()));

    // User prompt: VS Code Copilot sends it as copilot_chat.user_request;
    // fall back to GenAI semantic-convention keys if other IDEs are used.
    // Filter out tool_result payloads (Anthropic agentic loop sends "[{\"type\":\"tool_result\"...}]"
    // as the "user turn" for subsequent LLM calls — we only want the actual human text).
    let user_prompt = json_attr_str(attrs_slice, "copilot_chat.user_request")
        .filter(|s| !s.trim_start().starts_with("[{"))
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.prompt"))
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.prompt.0.content"))
        .or_else(|| json_user_message_content(attrs_slice))
        .or_else(|| extract_first_human_text(attrs_slice));

    let mut metadata = serde_json::json!({ "span_type": "chat" });
    if let Some(svc) = service_name { metadata["service_name"] = serde_json::json!(svc); }
    if let Some(sid) = session_id { metadata["session_id"] = serde_json::json!(sid); }

    Ok(ToolCallEvent {
        event_id: uuid::Uuid::new_v4(),
        task_id: None,
        repo: None,
        branch: None,
        ide: ide.map(|s| s.to_string()).or_else(|| Some("copilot-vscode".into())),
        agent: agent_name,
        skill: None,
        mcp_server,
        tool_name: "llm_chat".to_string(),
        started_at: start,
        ended_at: end,
        ok,
        error,
        request_bytes: None,
        response_bytes: None,
        estimated_input_tokens: input_tokens.map(|t| t as i32),
        estimated_output_tokens: output_tokens.map(|t| t as i32),
        request_sha256: None,
        response_sha256: None,
        metadata: Some(metadata),
        model,
        cached_tokens: cached_tokens.map(|t| t as i32),
        conversation_id,
        client_ip: client_ip.map(|s| s.to_string()),
        user_agent: user_agent.map(|s| s.to_string()),
        user_prompt,
    })
}

/// Infer IDE name from HTTP User-Agent and/or service.name
fn infer_ide(user_agent: Option<&str>, service_name: Option<&str>) -> Option<String> {
    let ua = user_agent.unwrap_or("").to_lowercase();
    let svc = service_name.unwrap_or("").to_lowercase();
    if ua.contains("vscode") || svc.contains("copilot") || svc.contains("vscode") {
        return Some("copilot-vscode".to_string());
    }
    if ua.contains("cursor") || svc.contains("cursor") {
        return Some("cursor".to_string());
    }
    if ua.contains("antigravity") || svc.contains("antigravity") {
        return Some("antigravity".to_string());
    }
    if ua.contains("opencode") || svc.contains("opencode") {
        return Some("opencode".to_string());
    }
    if ua.contains("rust-rover") || svc.contains("codex") {
        return Some("codex".to_string());
    }
    None
}

fn json_attr_str(attrs: &[serde_json::Value], key: &str) -> Option<String> {
    attrs.iter()
        .find(|kv| kv.get("key").and_then(|k| k.as_str()) == Some(key))
        .and_then(|kv| kv.pointer("/value/stringValue").and_then(|v| v.as_str()).map(|s| s.to_string()))
}

/// Finds the first message with role="user" among indexed prompt attributes
/// (gen_ai.prompt.{N}.role / gen_ai.prompt.{N}.content convention).
fn json_user_message_content(attrs: &[serde_json::Value]) -> Option<String> {
    // Collect all (index, role/content) pairs
    for i in 0..16usize {
        let role_key = format!("gen_ai.prompt.{i}.role");
        let content_key = format!("gen_ai.prompt.{i}.content");
        let role = json_attr_str(attrs, &role_key);
        if role.as_deref() == Some("user") {
            if let Some(content) = json_attr_str(attrs, &content_key) {
                return Some(content);
            }
        }
    }
    None
}

/// Parses the first human text message from a gen_ai.input.messages JSON string.
/// Skips messages whose content is an array (tool_result payloads from agentic loops).
fn parse_first_human_text(raw: &str) -> Option<String> {
    let msgs: serde_json::Value = serde_json::from_str(raw).ok()?;
    let arr = msgs.as_array()?;
    for msg in arr {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
        if role != "user" { continue; }
        let content = msg.get("content")?;
        // Only accept plain string content — skip arrays (tool_result payloads)
        if let Some(text) = content.as_str() {
            let trimmed = text.trim();
            if !trimmed.is_empty() && !trimmed.starts_with("[{") {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Extracts the first human text from gen_ai.input.messages attribute (JSON variant).
fn extract_first_human_text(attrs: &[serde_json::Value]) -> Option<String> {
    let raw = json_attr_str(attrs, "gen_ai.input.messages")?;
    parse_first_human_text(&raw)
}

fn json_attr_int(attrs: &[serde_json::Value], key: &str) -> Option<i64> {
    attrs.iter()
        .find(|kv| kv.get("key").and_then(|k| k.as_str()) == Some(key))
        .and_then(|kv| {
            let v = kv.get("value")?;
            v.get("intValue").and_then(|i| i.as_i64())
                .or_else(|| v.get("doubleValue").and_then(|d| d.as_f64()).map(|f| f as i64))
        })
}

fn handle_trace_request_proto(body: &[u8], client_ip: Option<&str>, user_agent: Option<&str>, pool: &PgPool) -> Result<Vec<serde_json::Value>, AppError> {
    let req = ExportTraceServiceRequest::decode(body)
        .map_err(|e| AppError::Validation(format!("failed to decode OTLP protobuf: {e}")))?;

    let mut results = Vec::new();

    for rs in &req.resource_spans {
        let resource_attrs = rs.resource.as_ref().map(|r| &r.attributes[..]).unwrap_or(&[]);
        let service_name = get_attr_str(resource_attrs, "service.name");
        let session_id = get_attr_str(resource_attrs, "session.id");
        let ide = infer_ide(user_agent, service_name.as_deref());

        for ss in &rs.scope_spans {
            for span in &ss.spans {
                if span.name.starts_with("execute_tool") {
                    let tool_hint = span.name.strip_prefix("execute_tool").map(|s| s.trim()).filter(|s| !s.is_empty());
                    match map_tool_call(span, resource_attrs, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, tool_hint) {
                        Ok(event) => {
                            match tokio::task::block_in_place(|| {
                                tokio::runtime::Handle::current().block_on(async {
                                    event_service::insert_tool_call(pool, event).await
                                })
                            }) {
                                Ok(record) => {
                                    results.push(serde_json::json!({
                                        "event_id": record.event_id,
                                        "tool_name": record.tool_name,
                                        "duration_ms": record.duration_ms,
                                    }));
                                }
                                Err(e) => warn!("failed to insert OTLP tool_call: {e}"),
                            }
                        }
                        Err(e) => warn!("failed to map OTLP execute_tool span: {e}"),
                    }
                } else if span.name.starts_with("chat") {
                    let model_hint = span.name.strip_prefix("chat").map(|s| s.trim()).filter(|s| !s.is_empty());
                    match map_chat_span_proto(span, resource_attrs, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, model_hint) {
                        Ok(event) => {
                            match tokio::task::block_in_place(|| {
                                tokio::runtime::Handle::current().block_on(async {
                                    event_service::insert_tool_call(pool, event).await
                                })
                            }) {
                                Ok(record) => {
                                    results.push(serde_json::json!({
                                        "event_id": record.event_id,
                                        "tool_name": record.tool_name,
                                        "duration_ms": record.duration_ms,
                                    }));
                                }
                                Err(e) => warn!("failed to insert OTLP chat span: {e}"),
                            }
                        }
                        Err(e) => warn!("failed to map OTLP chat span: {e}"),
                    }
                } else if span.name.starts_with("invoke_agent") {
                    // agent session boundary — skip
                } else {
                    warn!("unknown OTLP span name: {}", span.name);
                }
            }
        }
    }

    Ok(results)
}

fn map_tool_call(
    span: &Span,
    _resource_attrs: &[KeyValue],
    service_name: Option<&str>,
    session_id: Option<&str>,
    ide: Option<&str>,
    client_ip: Option<&str>,
    user_agent: Option<&str>,
    tool_name_hint: Option<&str>,
) -> Result<ToolCallEvent, &'static str> {
    let tool_name = get_attr_str(&span.attributes, "gen_ai.tool.name")
        .or_else(|| tool_name_hint.map(|s| s.to_string()))
        .unwrap_or_else(|| span.name.clone());

    let agent_name = get_attr_str(&span.attributes, "gen_ai.agent.name")
        .or_else(|| service_name.map(|s| s.to_string()));

    let start = chrono::DateTime::from_timestamp_nanos(span.start_time_unix_nano as i64);
    let end = chrono::DateTime::from_timestamp_nanos(span.end_time_unix_nano as i64);

    let ok = span.status.as_ref().map(|s| s.code != 2).unwrap_or(true);
    let error = if !ok {
        span.status.as_ref().map(|s| s.message.clone())
    } else {
        None
    };

    let input_tokens = get_attr_int(&span.attributes, "gen_ai.usage.input_tokens")
        .or_else(|| get_attr_int(&span.attributes, "gen_ai.usage.prompt_tokens"));
    let output_tokens = get_attr_int(&span.attributes, "gen_ai.usage.output_tokens")
        .or_else(|| get_attr_int(&span.attributes, "gen_ai.usage.completion_tokens"));
    let cached_tokens = get_attr_int(&span.attributes, "gen_ai.usage.cache_read.input_tokens")
        .or_else(|| get_attr_int(&span.attributes, "gen_ai.usage.cache_read_input_tokens"))
        .or_else(|| get_attr_int(&span.attributes, "gen_ai.usage.cached_tokens"))
        .or_else(|| get_attr_int(&span.attributes, "gen_ai.usage.input_tokens_cached"));
    let model = get_attr_str(&span.attributes, "gen_ai.response.model")
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.request.model"));
    let conversation_id = get_attr_str(&span.attributes, "gen_ai.conversation.id")
        .or_else(|| get_attr_str(&span.attributes, "copilot_chat.chat_session_id"))
        .or_else(|| get_attr_str(&span.attributes, "copilot.conversation.id"))
        .or_else(|| get_attr_str(&span.attributes, "thread.id"))
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.thread.id"))
        .or_else(|| session_id.map(|s| s.to_string()));

    let user_prompt = get_attr_str(&span.attributes, "copilot_chat.user_request")
        .filter(|s| !s.trim_start().starts_with("[{"))
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.prompt"))
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.prompt.0.content"))
        .or_else(|| {
            let raw = get_attr_str(&span.attributes, "gen_ai.input.messages")?;
            parse_first_human_text(&raw)
        });

    let mut metadata = serde_json::json!({});
    if let Some(svc) = service_name { metadata["service_name"] = serde_json::json!(svc); }
    if let Some(sid) = session_id { metadata["session_id"] = serde_json::json!(sid); }
    if let Some(agent) = &agent_name { metadata["agent"] = serde_json::json!(agent); }

    let event = ToolCallEvent {
        event_id: uuid::Uuid::new_v4(),
        task_id: None,
        repo: None,
        branch: None,
        ide: ide.map(|s| s.to_string()).or_else(|| Some("copilot-vscode".into())),
        agent: agent_name,
        skill: None,
        mcp_server: None,
        tool_name,
        started_at: start,
        ended_at: end,
        ok,
        error,
        request_bytes: None,
        response_bytes: None,
        estimated_input_tokens: input_tokens.map(|t| t as i32),
        estimated_output_tokens: output_tokens.map(|t| t as i32),
        request_sha256: None,
        response_sha256: None,
        metadata: Some(metadata),
        model,
        cached_tokens: cached_tokens.map(|t| t as i32),
        conversation_id,
        client_ip: client_ip.map(|s| s.to_string()),
        user_prompt,
        user_agent: user_agent.map(|s| s.to_string()),
    };

    Ok(event)
}

fn map_chat_span_proto(
    span: &Span,
    _resource_attrs: &[KeyValue],
    service_name: Option<&str>,
    session_id: Option<&str>,
    ide: Option<&str>,
    client_ip: Option<&str>,
    user_agent: Option<&str>,
    model_hint: Option<&str>,
) -> Result<ToolCallEvent, &'static str> {
    let start = chrono::DateTime::from_timestamp_nanos(span.start_time_unix_nano as i64);
    let end = chrono::DateTime::from_timestamp_nanos(span.end_time_unix_nano as i64);
    let ok = span.status.as_ref().map(|s| s.code != 2).unwrap_or(true);
    let error = if !ok { span.status.as_ref().map(|s| s.message.clone()) } else { None };

    let input_tokens = get_attr_int(&span.attributes, "gen_ai.usage.input_tokens")
        .or_else(|| get_attr_int(&span.attributes, "gen_ai.usage.prompt_tokens"));
    let output_tokens = get_attr_int(&span.attributes, "gen_ai.usage.output_tokens")
        .or_else(|| get_attr_int(&span.attributes, "gen_ai.usage.completion_tokens"));
    let cached_tokens = get_attr_int(&span.attributes, "gen_ai.usage.cache_read.input_tokens")
        .or_else(|| get_attr_int(&span.attributes, "gen_ai.usage.cache_read_input_tokens"))
        .or_else(|| get_attr_int(&span.attributes, "gen_ai.usage.cached_tokens"))
        .or_else(|| get_attr_int(&span.attributes, "gen_ai.usage.input_tokens_cached"));
    let model = get_attr_str(&span.attributes, "gen_ai.response.model")
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.request.model"))
        .or_else(|| model_hint.map(|s| s.to_string()));
    let system = get_attr_str(&span.attributes, "gen_ai.system");
    let conversation_id = get_attr_str(&span.attributes, "gen_ai.conversation.id")
        .or_else(|| get_attr_str(&span.attributes, "copilot_chat.chat_session_id"))
        .or_else(|| get_attr_str(&span.attributes, "copilot.conversation.id"))
        .or_else(|| get_attr_str(&span.attributes, "thread.id"))
        .or_else(|| session_id.map(|s| s.to_string()));
    let agent_name = get_attr_str(&span.attributes, "gen_ai.agent.name")
        .or_else(|| service_name.map(|s| s.to_string()));
    let mcp_server = system.or_else(|| {
        model.as_ref().map(|m| {
            if m.contains("claude") { "anthropic".to_string() }
            else if m.contains("gpt") || m.contains("o1") || m.contains("o3") { "openai".to_string() }
            else if m.contains("gemini") { "google".to_string() }
            else { "unknown".to_string() }
        })
    });
    let user_prompt = get_attr_str(&span.attributes, "copilot_chat.user_request")
        .filter(|s| !s.trim_start().starts_with("[{"))
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.prompt"))
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.prompt.0.content"))
        .or_else(|| {
            let raw = get_attr_str(&span.attributes, "gen_ai.input.messages")?;
            parse_first_human_text(&raw)
        });
    let mut metadata = serde_json::json!({ "span_type": "chat" });
    if let Some(svc) = service_name { metadata["service_name"] = serde_json::json!(svc); }
    if let Some(sid) = session_id { metadata["session_id"] = serde_json::json!(sid); }

    Ok(ToolCallEvent {
        event_id: uuid::Uuid::new_v4(),
        task_id: None, repo: None, branch: None,
        ide: ide.map(|s| s.to_string()).or_else(|| Some("antigravity".into())),
        agent: agent_name, skill: None, mcp_server,
        tool_name: "llm_chat".to_string(),
        started_at: start, ended_at: end, ok, error,
        request_bytes: None, response_bytes: None,
        estimated_input_tokens: input_tokens.map(|t| t as i32),
        estimated_output_tokens: output_tokens.map(|t| t as i32),
        request_sha256: None, response_sha256: None,
        metadata: Some(metadata), model,
        cached_tokens: cached_tokens.map(|t| t as i32),
        conversation_id,
        client_ip: client_ip.map(|s| s.to_string()),
        user_agent: user_agent.map(|s| s.to_string()),
        user_prompt,
    })
}

fn get_attr_str<'a>(attrs: &'a [KeyValue], key: &str) -> Option<String> {
    attrs.iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| kv.value.as_ref())
        .and_then(|v| match &v.value {
            Some(any_value::Value::StringValue(s)) => Some(s.clone()),
            _ => None,
        })
}

fn get_attr_int(attrs: &[KeyValue], key: &str) -> Option<i64> {
    attrs.iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| kv.value.as_ref())
        .and_then(|v| match &v.value {
            Some(any_value::Value::IntValue(n)) => Some(*n),
            _ => None,
        })
}

include!("proto.rs");
