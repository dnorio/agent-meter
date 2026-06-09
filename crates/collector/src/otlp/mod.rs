use prost::Message;
use sqlx::PgPool;
use tracing::warn;

mod ide;

use self::ide::infer_ide;

use crate::errors::AppError;
use crate::models::event::ToolCallEvent;
use crate::services::event_service;
use crate::services::ingest_buffer::IngestBuffer;

/// Persist a parsed event: buffer (fire-and-forget) or sync insert (fallback).
fn persist_event(
    pool: &PgPool,
    buffer: Option<&IngestBuffer>,
    event: ToolCallEvent,
    results: &mut Vec<serde_json::Value>,
    label: &str,
) {
    if let Some(buf) = buffer {
        let tool_name = event.tool_name.clone();
        match buf.try_send(event) {
            Ok(()) => {
                results.push(serde_json::json!({
                    "buffered": true,
                    "tool_name": tool_name,
                }));
            }
            Err(e) => warn!("ingest buffer full ({label}), dropping event: {e}"),
        }
    } else {
        // Legacy sync path (fallback when buffer is None)
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
            Err(e) => warn!("failed to insert {label}: {e}"),
        }
    }
}

/// Dispatcher: selects protobuf or JSON parser based on Content-Type.
pub fn handle_trace_request(
    body: &[u8],
    content_type: Option<&str>,
    client_ip: Option<&str>,
    user_agent: Option<&str>,
    pool: &PgPool,
    buffer: Option<&IngestBuffer>,
) -> Result<Vec<serde_json::Value>, AppError> {
    if content_type.map(|ct| ct.contains("application/json")).unwrap_or(false) {
        return handle_trace_request_json(body, client_ip, user_agent, pool, buffer);
    }
    handle_trace_request_proto(body, client_ip, user_agent, pool, buffer)
}

/// Parse OTLP JSON (format sent by VS Code / OpenTelemetry JS SDK).
fn handle_trace_request_json(body: &[u8], client_ip: Option<&str>, user_agent: Option<&str>, pool: &PgPool, buffer: Option<&IngestBuffer>) -> Result<Vec<serde_json::Value>, AppError> {
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

                // VS Code sends: "execute_tool <tool_name>", "chat <model>", "invoke_agent <agent>"
                if span_name.starts_with("execute_tool") {
                    let tool_hint = span_name.strip_prefix("execute_tool").map(|s| s.trim()).filter(|s| !s.is_empty());
                    match map_tool_call_json(span, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, tool_hint) {
                        Ok(event) => {
                            persist_event(pool, buffer, event, &mut results, "JSON OTLP tool_call");
                        }
                        Err(e) => warn!("failed to map JSON OTLP execute_tool span: {e}"),
                    }
                } else if span_name.starts_with("chat") {
                    // "chat <model>" spans: LLM completion with token accounting
                    let model_hint = span_name.strip_prefix("chat").map(|s| s.trim()).filter(|s| !s.is_empty());
                    match map_chat_span_json(span, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, model_hint) {
                        Ok(event) => {
                            persist_event(pool, buffer, event, &mut results, "JSON OTLP chat");
                        }
                        Err(e) => warn!("failed to map JSON OTLP chat span: {e}"),
                    }
                } else if span_name.starts_with("invoke_agent") {
                    // "invoke_agent <agent_name>" — agent session boundary, ignore for now
                } else if span_name.starts_with("tools/call") || span_name.starts_with("tools/notify") {
                    // MCP OTel semconv (new standard): span name = "tools/call <tool_name>"
                    // see: opentelemetry.io/docs/specs/semconv/gen-ai/mcp/
                    let tool_hint = span_name.strip_prefix("tools/call").map(|s| s.trim()).filter(|s| !s.is_empty());
                    match map_tool_call_json(span, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, tool_hint) {
                        Ok(event) => {
                            persist_event(pool, buffer, event, &mut results, "JSON OTLP mcp tools/call");
                        }
                        Err(e) => warn!("failed to map JSON OTLP tools/call span: {e}"),
                    }
                } else if is_copilot_http_span(span, span_name) {
                    // HTTP spans from javaagent (approach 3: eclipse.ini instrumentation)
                    // These are HTTP client calls to GitHub Copilot API endpoints
                    let (tool_name, is_chat) = classify_copilot_http_span(span);
                    if is_chat {
                        match map_chat_span_json(span, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, None) {
                            Ok(mut event) => {
                                event.tool_name = tool_name;
                                persist_event(pool, buffer, event, &mut results, "copilot HTTP chat");
                            }
                            Err(e) => warn!("failed to map copilot HTTP chat span: {e}"),
                        }
                    } else {
                        match map_tool_call_json(span, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, Some(&tool_name)) {
                            Ok(event) => {
                                persist_event(pool, buffer, event, &mut results, "copilot HTTP tool");
                            }
                            Err(e) => warn!("failed to map copilot HTTP tool span: {e}"),
                        }
                    }
                } else {
                    // truly unknown span — skip silently for non-copilot services
                    if service_name.as_deref().map(|s| s.contains("copilot") || s.contains("eclipse")).unwrap_or(false) {
                        warn!("unknown OTLP span name from copilot service (json): {}", span_name);
                    }
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
        .or_else(|| json_attr_str(attrs_slice, "mcp.server"))
        .or_else(|| infer_mcp_server_from_tool(&tool_name));

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
    // mcp.session.id is the MCP OTel semconv session identifier (tools/call spans)
    let trace_id = span.get("traceId").and_then(|v| v.as_str()).map(|s| s.to_string());
    let conversation_id = json_attr_str(attrs_slice, "gen_ai.conversation.id")
        .or_else(|| json_attr_str(attrs_slice, "copilot_chat.chat_session_id"))
        .or_else(|| json_attr_str(attrs_slice, "copilot.conversation.id"))
        .or_else(|| json_attr_str(attrs_slice, "thread.id"))
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.thread.id"))
        .or_else(|| json_attr_str(attrs_slice, "mcp.session.id"))  // MCP OTel semconv
        .or_else(|| session_id.map(|s| s.to_string()))
        .or(trace_id);
    let response_bytes = json_attr_int(attrs_slice, "gen_ai.response.bytes");
    let request_bytes = json_attr_int(attrs_slice, "gen_ai.request.bytes");

    // Tool arguments (input) and result (output) — sent by some agents/IDEs
    // GenAI semantic convention draft: gen_ai.tool.call.id, gen_ai.tool.output
    // Also check copilot-specific and generic keys
    let tool_arguments: Option<serde_json::Value> = json_attr_str(attrs_slice, "gen_ai.tool.call.arguments")
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.tool.input"))
        .or_else(|| json_attr_str(attrs_slice, "tool.input"))
        .and_then(|s| serde_json::from_str(&s).ok());

    let tool_result: Option<String> = json_attr_str(attrs_slice, "gen_ai.tool.call.result")  // MCP OTel semconv (new)
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.tool.output"))
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.tool.result"))
        .or_else(|| json_attr_str(attrs_slice, "tool.output"))
        .map(|s| truncate_str(s, 8 * 1024));

    let user_prompt = extract_clean_user_prompt_json(attrs_slice);

    let mut metadata = serde_json::json!({});
    if let Some(svc) = service_name { metadata["service_name"] = serde_json::json!(svc); }
    if let Some(sid) = session_id { metadata["session_id"] = serde_json::json!(sid); }
    if let Some(a) = &agent_name { metadata["agent"] = serde_json::json!(a); }

    // T-332: deep telemetry fields for map_tool_call_json
    let trace_id = span.get("traceId").and_then(|v| v.as_str()).map(|s| s.to_string());
    let span_id = span.get("spanId").and_then(|v| v.as_str()).map(|s| s.to_string());
    let parent_span_id = span.get("parentSpanId").and_then(|v| v.as_str()).map(|s| s.to_string());
    let tool_call_id = json_attr_str(attrs_slice, "gen_ai.tool.call.id");
    let reasoning_tokens = json_attr_int(attrs_slice, "gen_ai.usage.reasoning_tokens");
    let finish_reason: Option<String> = json_attr_str(attrs_slice, "gen_ai.response.finish_reason")
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.response.finish_reasons")
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()
                .and_then(|v| v.into_iter().next())
                .or_else(|| serde_json::from_str::<serde_json::Value>(&s).ok()
                    .and_then(|v| v.as_array()?.first()?.as_str().map(|s| s.to_string())))));
    let request_max_tokens = json_attr_int(attrs_slice, "gen_ai.request.max_tokens").map(|t| t as i32);
    let request_temperature = json_attr_float(attrs_slice, "gen_ai.request.temperature");
    let llm_system = json_attr_str(attrs_slice, "gen_ai.system");

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
        tool_arguments,
        tool_result,
        reasoning_tokens: reasoning_tokens.map(|t| t as i32),
        finish_reason,
        request_max_tokens,
        request_temperature,
        llm_system,
        trace_id,
        span_id,
        parent_span_id,
        tool_call_id,
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

    let user_prompt = extract_clean_user_prompt_json(attrs_slice);

    let mut metadata = serde_json::json!({ "span_type": "chat" });
    if let Some(svc) = service_name { metadata["service_name"] = serde_json::json!(svc); }
    if let Some(sid) = session_id { metadata["session_id"] = serde_json::json!(sid); }

    // T-332: deep telemetry for map_chat_span_json
    let trace_id_val = span.get("traceId").and_then(|v| v.as_str()).map(|s| s.to_string());
    let span_id = span.get("spanId").and_then(|v| v.as_str()).map(|s| s.to_string());
    let parent_span_id = span.get("parentSpanId").and_then(|v| v.as_str()).map(|s| s.to_string());
    let reasoning_tokens = json_attr_int(attrs_slice, "gen_ai.usage.reasoning_tokens");
    let finish_reason: Option<String> = json_attr_str(attrs_slice, "gen_ai.response.finish_reason")
        .or_else(|| json_attr_str(attrs_slice, "gen_ai.response.finish_reasons")
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()
                .and_then(|v| v.into_iter().next())
                .or_else(|| serde_json::from_str::<serde_json::Value>(&s).ok()
                    .and_then(|v| v.as_array()?.first()?.as_str().map(|s| s.to_string())))));
    let request_max_tokens = json_attr_int(attrs_slice, "gen_ai.request.max_tokens").map(|t| t as i32);
    let request_temperature = json_attr_float(attrs_slice, "gen_ai.request.temperature");
    let llm_system = json_attr_str(attrs_slice, "gen_ai.system");
    // conversation_id already falls back to trace_id above
    let chat_trace_id = trace_id_val;

    // LLM response text: extract from output messages or completion attributes
    let response_text = extract_response_text(attrs_slice);

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
        tool_arguments: None,
        tool_result: response_text,
        reasoning_tokens: reasoning_tokens.map(|t| t as i32),
        finish_reason,
        request_max_tokens,
        request_temperature,
        llm_system,
        trace_id: chat_trace_id,
        span_id,
        parent_span_id,
        tool_call_id: None,
    })
}

/// Infer `mcp_server` from VS Code built-in tool name patterns.
/// VS Code doesn't send `mcp.server.name` in execute_tool spans.
fn infer_mcp_server_from_tool(tool_name: &str) -> Option<String> {
    // MCP servers that prefix their tools with "mcp_<server>_"
    if let Some(rest) = tool_name.strip_prefix("mcp_") {
        let server = rest.split('_').next().unwrap_or(rest);
        // Normalize known names
        let normalized = match server {
            "chromedevtool" | "chromedevtools" | "chrome" => "chromeDevtools",
            "gitkraken" => "gitkraken",
            "playwright" => "playwright",
            "filesystem" => "filesystem",
            _ => server,
        };
        return Some(normalized.to_string());
    }
    // VS Code built-in tools
    let builtin = [
        "run_in_terminal", "read_file", "replace_string_in_file", "create_file",
        "grep_search", "file_search", "list_dir", "semantic_search", "get_errors",
        "manage_todo_list", "view_image", "run_in_terminal", "get_terminal_output",
        "kill_terminal", "send_to_terminal", "vscode_askQuestions", "vscode_listCodeUsages",
        "vscode_renameSymbol", "multi_replace_string_in_file", "tool_search", "runSubagent",
        "open_browser_page", "click_element", "hover_element", "navigate_page",
        "screenshot_page", "read_page",
    ];
    if builtin.contains(&tool_name) {
        return Some("vscode-builtin".to_string());
    }
    // Memory / session tools
    if tool_name == "memory" || tool_name.starts_with("memory_") {
        return Some("copilot-memory".to_string());
    }
    // fetch/web tools
    if tool_name == "fetch_webpage" || tool_name.starts_with("fetch_") {
        return Some("fetch".to_string());
    }
    None
}

/// Truncate a string at max_bytes on a valid UTF-8 boundary.
fn truncate_str(s: String, max_bytes: usize) -> String {
    if s.len() <= max_bytes { return s; }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) { end -= 1; }
    format!("{}…[truncated]", &s[..end])
}

fn json_attr_str(attrs: &[serde_json::Value], key: &str) -> Option<String> {
    attrs.iter()
        .find(|kv| kv.get("key").and_then(|k| k.as_str()) == Some(key))
        .and_then(|kv| kv.pointer("/value/stringValue").and_then(|v| v.as_str()).map(|s| s.to_string()))
}

/// Finds the first message with role="user" among indexed prompt attributes
/// (gen_ai.prompt.{N}.role / gen_ai.prompt.{N}.content convention).
/// Parses the first human text message from a gen_ai.input.messages JSON string.
/// Handles multiple content formats sent by VS Code Copilot:
///
///   1. Gemini/Copilot format (parts array with "content" field):
///      {"role":"user","parts":[{"type":"text","content":"<userRequest>prompt</userRequest>..."}]}
///      The actual user text may be wrapped in <userRequest>…</userRequest> tags.
///
///   2. Anthropic block array with "text" field:
///      {"role":"user","content":[{"type":"text","text":"prompt"}]}
///      Skips if the first block is a "tool_result" (agentic loop turn).
///
///   3. Plain string content:
///      {"role":"user","content":"prompt"}
fn parse_first_human_text(raw: &str) -> Option<String> {
    let msgs: serde_json::Value = serde_json::from_str(raw).ok()?;
    let arr = msgs.as_array()?;

    for msg in arr {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
        if role != "user" { continue; }

        // Format 1: "parts" array (Copilot/Gemini format)
        // VS Code Copilot sends multiple parts per user message:
        //   parts[0]: context blocks (<environment_info>, <workspace_info>, <availableDeferredTools>, …)
        //   parts[1]: <conversation-summary>…</conversation-summary>
        //   parts[N]: the actual user text (plain, no XML wrapper)
        // Strategy: iterate in REVERSE to find the last part whose content is plain text.
        if let Some(parts) = msg.get("parts").and_then(|p| p.as_array()) {
            // Pass 1: find the last text-type part that is NOT a context XML block
            for part in parts.iter().rev() {
                if part.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(content) = part.get("content").and_then(|c| c.as_str()) {
                        let trimmed = content.trim();
                        // Context blocks start with '<' — but check for <userRequest> tag first
                        if trimmed.starts_with('<') {
                            if let Some(extracted) = extract_user_request_tag(trimmed) {
                                return Some(extracted);
                            }
                            continue;
                        }
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
            continue; // tried "parts", skip "content" variants
        }

        // Format 2: plain string content
        if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
            let trimmed = text.trim();
            if trimmed.starts_with('<') || trimmed.starts_with("[{") {
                if let Some(extracted) = extract_user_request_tag(trimmed) {
                    return Some(extracted);
                }
                continue;
            }
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
            continue;
        }

        // Format 3: Anthropic block array
        if let Some(blocks) = msg.get("content").and_then(|c| c.as_array()) {
            // Skip if first block is tool_result (agentic loop turn)
            let first_type = blocks.first()
                .and_then(|b| b.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("");
            if first_type == "tool_result" { continue; }

            for block in blocks {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        let trimmed = text.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Returns true if raw prompt text looks like noise/context rather than a real user message.
fn is_noise_prompt(s: &str) -> bool {
    let t = s.trim();
    // Reject JSON blobs
    t.starts_with('[')
        || t.starts_with('{')
        // Reject XML system prompts (but not short XML tags like <current_datetime>)
        || (t.starts_with('<') && !t.starts_with("<current_datetime>") && t.len() > 500)
        || t.starts_with("The current date")
        || t.starts_with("Terminals:")
        || t.starts_with("[Terminal")
        || t.starts_with("You are ")
        || t.len() > 2000
        || t.to_ascii_lowercase().starts_with("summarize the following")
        || t.to_ascii_lowercase().starts_with("please write a brief title")
}

/// Clean extraction of the actual user-typed prompt from OTLP attributes.
/// Priority: copilot_chat.user_request → <userRequest> inside gen_ai.prompt →
/// clean gen_ai.prompt → gen_ai.prompt.0.content → message arrays → input.messages.
fn extract_clean_user_prompt_json(attrs: &[serde_json::Value]) -> Option<String> {
    // 1. VS Code explicit attribute (best source)
    if let Some(s) = json_attr_str(attrs, "copilot_chat.user_request") {
        let trimmed = s.trim();
        if trimmed.starts_with("[{") || trimmed.starts_with("{") {
            // JSON array/object — parse as messages and extract user text
            if let Some(extracted) = parse_first_human_text(trimmed) {
                if !is_noise_prompt(&extracted) {
                    return Some(extracted);
                }
            }
        } else if !is_noise_prompt(&s) {
            return Some(s);
        }
    }

    // 2. gen_ai.input.messages — primary source for VS Code Copilot chat spans
    if let Some(raw) = json_attr_str(attrs, "gen_ai.input.messages") {
        if let Some(extracted) = parse_first_human_text(&raw) {
            if !is_noise_prompt(&extracted) {
                return Some(extracted);
            }
        }
    }

    // 3. gen_ai.prompt — try to extract <userRequest> tag first
    if let Some(raw) = json_attr_str(attrs, "gen_ai.prompt") {
        if let Some(extracted) = extract_user_request_tag(&raw) {
            return Some(extracted);
        }
        if !is_noise_prompt(&raw) {
            return Some(raw);
        }
    }

    // 4. gen_ai.prompt.N.content (indexed messages) — scan for user role
    for i in 0..20 {
        let role_key = format!("gen_ai.prompt.{i}.role");
        let content_key = format!("gen_ai.prompt.{i}.content");
        if let Some(role) = json_attr_str(attrs, &role_key) {
            if role == "user" {
                if let Some(content) = json_attr_str(attrs, &content_key) {
                    if let Some(extracted) = extract_user_request_tag(&content) {
                        return Some(extracted);
                    }
                    if !is_noise_prompt(&content) {
                        return Some(content);
                    }
                }
            }
        } else {
            break;
        }
    }

    None
}

/// Same as extract_clean_user_prompt_json but for proto-decoded spans with get_attr_str.
fn extract_clean_user_prompt_proto(attrs: &[KeyValue]) -> Option<String> {
    if let Some(s) = get_attr_str(attrs, "copilot_chat.user_request") {
        let trimmed = s.trim();
        if trimmed.starts_with("[{") || trimmed.starts_with("{") {
            if let Some(extracted) = parse_first_human_text(trimmed) {
                if !is_noise_prompt(&extracted) {
                    return Some(extracted);
                }
            }
        } else if !is_noise_prompt(&s) {
            return Some(s);
        }
    }

    if let Some(raw) = get_attr_str(attrs, "gen_ai.input.messages") {
        if let Some(extracted) = parse_first_human_text(&raw) {
            if !is_noise_prompt(&extracted) {
                return Some(extracted);
            }
        }
    }

    if let Some(raw) = get_attr_str(attrs, "gen_ai.prompt") {
        if let Some(extracted) = extract_user_request_tag(&raw) {
            return Some(extracted);
        }
        if !is_noise_prompt(&raw) {
            return Some(raw);
        }
    }

    if let Some(s) = get_attr_str(attrs, "gen_ai.prompt.0.content") {
        if !is_noise_prompt(&s) {
            return Some(s);
        }
    }

    None
}

/// Extracts content between <userRequest>…</userRequest> tags, if present.
/// Only matches when the tag appears at the top level — skips occurrences
/// inside <conversation-summary> which may mention the tag as documentation.
fn extract_user_request_tag(content: &str) -> Option<String> {
    const START: &str = "<userRequest>";
    const END: &str = "</userRequest>";
    let start_idx = content.find(START)?;

    // Skip if the <userRequest> is inside a <conversation-summary> block
    // (it would be a documentation mention, not an actual tag)
    if let Some(cs_start) = content.find("<conversation-summary>") {
        if let Some(cs_end) = content.find("</conversation-summary>") {
            if start_idx > cs_start && start_idx < cs_end {
                return None;
            }
        }
    }

    let after_start = start_idx + START.len();
    let end_idx = content[after_start..].find(END)?;
    let text = content[after_start..after_start + end_idx].trim();
    if text.is_empty() || is_noise_prompt(text) { None } else { Some(text.to_string()) }
}

/// Extracts LLM response text from output/completion attributes.
/// Checks: gen_ai.output.messages (JSON array), gen_ai.completion.0.content,
/// gen_ai.response.text, copilot_chat.response (VS Code specific).
fn extract_response_text(attrs: &[serde_json::Value]) -> Option<String> {
    // 1. VS Code specific response attribute
    if let Some(text) = json_attr_str(attrs, "copilot_chat.response") {
        if !text.is_empty() {
            return Some(truncate_str(text, 8 * 1024));
        }
    }
    // 2. gen_ai.output.messages (JSON array of messages)
    if let Some(raw) = json_attr_str(attrs, "gen_ai.output.messages") {
        if let Ok(msgs) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(arr) = msgs.as_array() {
                for msg in arr {
                    let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
                    if role == "assistant" || role == "model" {
                        // Plain string content
                        if let Some(text) = msg.get("content").and_then(|c| c.as_str()) {
                            if !text.is_empty() {
                                return Some(truncate_str(text.to_string(), 8 * 1024));
                            }
                        }
                        // Block array content (Anthropic)
                        if let Some(blocks) = msg.get("content").and_then(|c| c.as_array()) {
                            for block in blocks {
                                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                        if !text.is_empty() {
                                            return Some(truncate_str(text.to_string(), 8 * 1024));
                                        }
                                    }
                                }
                            }
                        }
                        // Gemini parts format
                        if let Some(parts) = msg.get("parts").and_then(|p| p.as_array()) {
                            for part in parts {
                                if let Some(text) = part.get("content").and_then(|c| c.as_str())
                                    .or_else(|| part.get("text").and_then(|t| t.as_str())) {
                                    if !text.is_empty() {
                                        return Some(truncate_str(text.to_string(), 8 * 1024));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // 3. Indexed completion attributes (gen_ai.completion.0.content)
    for i in 0..4usize {
        let key = format!("gen_ai.completion.{i}.content");
        if let Some(text) = json_attr_str(attrs, &key) {
            if !text.is_empty() {
                return Some(truncate_str(text, 8 * 1024));
            }
        }
    }
    // 4. gen_ai.response.text (generic)
    if let Some(text) = json_attr_str(attrs, "gen_ai.response.text") {
        if !text.is_empty() {
            return Some(truncate_str(text, 8 * 1024));
        }
    }
    None
}

fn json_attr_int(attrs: &[serde_json::Value], key: &str) -> Option<i64> {
    attrs.iter()
        .find(|kv| kv.get("key").and_then(|k| k.as_str()) == Some(key))
        .and_then(|kv| {
            let v = kv.get("value")?;
            v.get("intValue").and_then(|i| {
                // OTLP JSON spec: int64 is serialized as string in protobuf3 JSON mapping
                i.as_i64().or_else(|| i.as_str().and_then(|s| s.parse::<i64>().ok()))
            })
                .or_else(|| v.get("doubleValue").and_then(|d| d.as_f64()).map(|f| f as i64))
                .or_else(|| v.get("stringValue").and_then(|s| s.as_str()).and_then(|s| s.parse::<i64>().ok()))
        })
}

fn json_attr_float(attrs: &[serde_json::Value], key: &str) -> Option<f64> {
    attrs.iter()
        .find(|kv| kv.get("key").and_then(|k| k.as_str()) == Some(key))
        .and_then(|kv| {
            let v = kv.get("value")?;
            v.get("doubleValue").and_then(|d| d.as_f64())
                .or_else(|| v.get("intValue").and_then(|i| i.as_i64()).map(|i| i as f64))
                .or_else(|| v.get("stringValue").and_then(|s| s.as_str()).and_then(|s| s.parse::<f64>().ok()))
        })
}

fn handle_trace_request_proto(body: &[u8], client_ip: Option<&str>, user_agent: Option<&str>, pool: &PgPool, buffer: Option<&IngestBuffer>) -> Result<Vec<serde_json::Value>, AppError> {
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
                            persist_event(pool, buffer, event, &mut results, "OTLP tool_call");
                        }
                        Err(e) => warn!("failed to map OTLP execute_tool span: {e}"),
                    }
                } else if span.name.starts_with("chat") {
                    let model_hint = span.name.strip_prefix("chat").map(|s| s.trim()).filter(|s| !s.is_empty());
                    match map_chat_span_proto(span, resource_attrs, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, model_hint) {
                        Ok(event) => {
                            persist_event(pool, buffer, event, &mut results, "OTLP chat");
                        }
                        Err(e) => warn!("failed to map OTLP chat span: {e}"),
                    }
                } else if span.name.starts_with("invoke_agent") {
                    // agent session boundary — skip
                } else if span.name.starts_with("tools/call") || span.name.starts_with("tools/notify") {
                    // MCP OTel semconv (new standard): span name = "tools/call <tool_name>"
                    let tool_hint = span.name.strip_prefix("tools/call").map(|s| s.trim()).filter(|s| !s.is_empty());
                    match map_tool_call(span, resource_attrs, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, tool_hint) {
                        Ok(event) => {
                            persist_event(pool, buffer, event, &mut results, "OTLP mcp tools/call");
                        }
                        Err(e) => warn!("failed to map OTLP tools/call span: {e}"),
                    }
                } else if is_copilot_http_span_proto(span) || is_eclipse_service_http_span(span, service_name.as_deref()) {
                    // HTTP spans from javaagent (approach 3: eclipse.ini instrumentation)
                    // Accept both: copilot-URL spans AND any HTTP span from eclipse service
                    let (tool_name, is_chat) = classify_copilot_http_span_proto(span);
                    if is_chat {
                        match map_chat_span_proto(span, resource_attrs, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, None) {
                            Ok(mut event) => {
                                event.tool_name = tool_name;
                                persist_event(pool, buffer, event, &mut results, "copilot HTTP chat (proto)");
                            }
                            Err(e) => warn!("failed to map copilot HTTP chat span (proto): {e}"),
                        }
                    } else {
                        match map_tool_call(span, resource_attrs, service_name.as_deref(), session_id.as_deref(), ide.as_deref(), client_ip, user_agent, Some(&tool_name)) {
                            Ok(event) => {
                                persist_event(pool, buffer, event, &mut results, "copilot HTTP tool (proto)");
                            }
                            Err(e) => warn!("failed to map copilot HTTP tool span (proto): {e}"),
                        }
                    }
                } else {
                    // truly unknown span — only warn if it looks relevant
                    let svc = service_name.as_deref().unwrap_or("");
                    if (svc.contains("copilot") || svc.contains("eclipse")) && !is_generic_http_method(&span.name) {
                        warn!("unknown OTLP span name from copilot service (proto): {}", span.name);
                    }
                }
            }
        }
    }

    Ok(results)
}

#[allow(clippy::too_many_arguments)]
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

    let mcp_server = get_attr_str(&span.attributes, "mcp.server.name")
        .or_else(|| get_attr_str(&span.attributes, "mcp.server"))
        .or_else(|| infer_mcp_server_from_tool(&tool_name));

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
        .or_else(|| get_attr_str(&span.attributes, "mcp.session.id"))  // MCP OTel semconv
        .or_else(|| session_id.map(|s| s.to_string()));

    let user_prompt = extract_clean_user_prompt_proto(&span.attributes);

    let mut metadata = serde_json::json!({});
    if let Some(svc) = service_name { metadata["service_name"] = serde_json::json!(svc); }
    if let Some(sid) = session_id { metadata["session_id"] = serde_json::json!(sid); }
    if let Some(agent) = &agent_name { metadata["agent"] = serde_json::json!(agent); }

    let tool_arguments: Option<serde_json::Value> = get_attr_str(&span.attributes, "gen_ai.tool.call.arguments")
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.tool.input"))
        .or_else(|| get_attr_str(&span.attributes, "tool.input"))
        .and_then(|s| serde_json::from_str(&s).ok());

    let tool_result: Option<String> = get_attr_str(&span.attributes, "gen_ai.tool.call.result")  // MCP OTel semconv (new)
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.tool.output"))
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.tool.result"))
        .or_else(|| get_attr_str(&span.attributes, "tool.output"))
        .map(|s| truncate_str(s, 8192));

    // T-332: deep telemetry for map_tool_call proto
    let trace_id = if span.trace_id.is_empty() { None } else { Some(hex::encode(&span.trace_id)) };
    let span_id = if span.span_id.is_empty() { None } else { Some(hex::encode(&span.span_id)) };
    let parent_span_id = if span.parent_span_id.is_empty() { None } else { Some(hex::encode(&span.parent_span_id)) };
    let tool_call_id = get_attr_str(&span.attributes, "gen_ai.tool.call.id");
    let reasoning_tokens = get_attr_int(&span.attributes, "gen_ai.usage.reasoning_tokens");
    let finish_reason = get_attr_str(&span.attributes, "gen_ai.response.finish_reason")
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.response.finish_reasons")
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()
                .and_then(|v| v.into_iter().next())));
    let request_max_tokens = get_attr_int(&span.attributes, "gen_ai.request.max_tokens").map(|t| t as i32);
    let request_temperature = get_attr_float(&span.attributes, "gen_ai.request.temperature");
    let llm_system = get_attr_str(&span.attributes, "gen_ai.system");

    let event = ToolCallEvent {
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
        tool_arguments,
        tool_result,
        reasoning_tokens: reasoning_tokens.map(|t| t as i32),
        finish_reason,
        request_max_tokens,
        request_temperature,
        llm_system,
        trace_id,
        span_id,
        parent_span_id,
        tool_call_id,
    };

    Ok(event)
}

#[allow(clippy::too_many_arguments)]
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
    let user_prompt = extract_clean_user_prompt_proto(&span.attributes);
    let mut metadata = serde_json::json!({ "span_type": "chat" });
    if let Some(svc) = service_name { metadata["service_name"] = serde_json::json!(svc); }
    if let Some(sid) = session_id { metadata["session_id"] = serde_json::json!(sid); }

    // T-332: deep telemetry for map_chat_span_proto
    let trace_id = if span.trace_id.is_empty() { None } else { Some(hex::encode(&span.trace_id)) };
    let span_id_val = if span.span_id.is_empty() { None } else { Some(hex::encode(&span.span_id)) };
    let parent_span_id = if span.parent_span_id.is_empty() { None } else { Some(hex::encode(&span.parent_span_id)) };
    let reasoning_tokens = get_attr_int(&span.attributes, "gen_ai.usage.reasoning_tokens");
    let finish_reason = get_attr_str(&span.attributes, "gen_ai.response.finish_reason")
        .or_else(|| get_attr_str(&span.attributes, "gen_ai.response.finish_reasons")
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()
                .and_then(|v| v.into_iter().next())));
    let request_max_tokens = get_attr_int(&span.attributes, "gen_ai.request.max_tokens").map(|t| t as i32);
    let request_temperature = get_attr_float(&span.attributes, "gen_ai.request.temperature");
    let llm_system = get_attr_str(&span.attributes, "gen_ai.system");

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
        tool_arguments: None,
        tool_result: None,
        reasoning_tokens: reasoning_tokens.map(|t| t as i32),
        finish_reason,
        request_max_tokens,
        request_temperature,
        llm_system,
        trace_id,
        span_id: span_id_val,
        parent_span_id,
        tool_call_id: None,
    })
}

fn get_attr_str(attrs: &[KeyValue], key: &str) -> Option<String> {
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

fn get_attr_float(attrs: &[KeyValue], key: &str) -> Option<f64> {
    attrs.iter()
        .find(|kv| kv.key == key)
        .and_then(|kv| kv.value.as_ref())
        .and_then(|v| match &v.value {
            Some(any_value::Value::DoubleValue(d)) => Some(*d),
            Some(any_value::Value::IntValue(i)) => Some(*i as f64),
            Some(any_value::Value::StringValue(s)) => s.parse::<f64>().ok(),
            _ => None,
        })
}

// ─── HTTP span detection for javaagent-instrumented Eclipse ────────────────

/// Checks if a span is an HTTP client span targeting Copilot/GitHub API.
/// The OTEL javaagent produces spans like "GET", "POST", "HTTP GET" etc.
/// with attributes: http.url, url.full, http.method, server.address
fn is_copilot_http_span(span: &serde_json::Value, span_name: &str) -> bool {
    // Quick check: span name should look like HTTP method
    let is_http_name = matches!(
        span_name.to_uppercase().as_str(),
        "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HTTP GET" | "HTTP POST" | "HTTP PUT"
    );
    if !is_http_name {
        return false;
    }

    // Check attributes for copilot-related URLs
    let attrs = span.get("attributes").and_then(|v| v.as_array());
    let Some(attrs) = attrs else { return false };

    for attr in attrs {
        let key = attr.get("key").and_then(|k| k.as_str()).unwrap_or("");
        if matches!(key, "http.url" | "url.full" | "server.address" | "http.target") {
            let val = attr.get("value")
                .and_then(|v| v.get("stringValue"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let lower = val.to_lowercase();
            if lower.contains("copilot") || lower.contains("githubcopilot")
                || lower.contains("api.github.com/copilot")
                || lower.contains("copilot-proxy")
                || lower.contains("default.exp-tas.com") // Copilot experiment service
            {
                return true;
            }
        }
    }
    false
}

/// Classifies a Copilot HTTP span into a tool_name and whether it's a chat span.
/// Returns (tool_name, is_chat).
fn classify_copilot_http_span(span: &serde_json::Value) -> (String, bool) {
    let attrs = span.get("attributes").and_then(|v| v.as_array());
    let url = attrs.and_then(|a| {
        a.iter().find_map(|attr| {
            let key = attr.get("key").and_then(|k| k.as_str()).unwrap_or("");
            if matches!(key, "http.url" | "url.full" | "http.target") {
                attr.get("value")
                    .and_then(|v| v.get("stringValue"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
    }).unwrap_or_default();

    let lower = url.to_lowercase();

    if lower.contains("/chat/completions") || lower.contains("/conversation") || lower.contains("/responses") {
        ("llm_chat".to_string(), true)
    } else if lower.contains("/completions") {
        ("copilot_completions".to_string(), false)
    } else if lower.contains("/telemetry") {
        ("copilot_telemetry".to_string(), false)
    } else if lower.contains("/models") {
        ("copilot_models".to_string(), false)
    } else if lower.contains("/token") || lower.contains("/oauth") {
        ("copilot_auth".to_string(), false)
    } else {
        ("copilot_api".to_string(), false)
    }
}

// ─── Eclipse service-level HTTP span detection ─────────────────────────────

/// Returns true if the span is an HTTP method AND the service_name contains "eclipse" or "copilot".
/// This catches ALL HTTP activity from the javaagent even when URLs aren't copilot-specific.
fn is_eclipse_service_http_span(span: &Span, service_name: Option<&str>) -> bool {
    let svc = service_name.unwrap_or("").to_lowercase();
    if !svc.contains("eclipse") && !svc.contains("copilot") {
        return false;
    }
    is_generic_http_method(&span.name)
}

fn is_generic_http_method(name: &str) -> bool {
    let upper = name.to_uppercase();
    matches!(
        upper.as_str(),
        "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS"
            | "HTTP GET" | "HTTP POST" | "HTTP PUT" | "HTTP DELETE"
    )
}

// ─── Protobuf equivalents for HTTP span detection ──────────────────────────

/// Proto version: checks if a protobuf Span is an HTTP client span targeting Copilot API.
fn is_copilot_http_span_proto(span: &Span) -> bool {
    let upper = span.name.to_uppercase();
    let is_http = matches!(
        upper.as_str(),
        "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HTTP GET" | "HTTP POST" | "HTTP PUT"
    );
    if !is_http {
        return false;
    }

    for kv in &span.attributes {
        if matches!(kv.key.as_str(), "http.url" | "url.full" | "server.address" | "http.target") {
            if let Some(val) = kv.value.as_ref().and_then(|v| match &v.value {
                Some(any_value::Value::StringValue(s)) => Some(s.as_str()),
                _ => None,
            }) {
                let lower = val.to_lowercase();
                if lower.contains("copilot") || lower.contains("githubcopilot")
                    || lower.contains("api.github.com/copilot")
                    || lower.contains("copilot-proxy")
                    || lower.contains("default.exp-tas.com")
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Proto version: classifies Copilot HTTP span.
fn classify_copilot_http_span_proto(span: &Span) -> (String, bool) {
    let url = span.attributes.iter().find_map(|kv| {
        if matches!(kv.key.as_str(), "http.url" | "url.full" | "http.target") {
            kv.value.as_ref().and_then(|v| match &v.value {
                Some(any_value::Value::StringValue(s)) => Some(s.clone()),
                _ => None,
            })
        } else {
            None
        }
    }).unwrap_or_default();

    let lower = url.to_lowercase();

    if lower.contains("/chat/completions") || lower.contains("/conversation") || lower.contains("/responses") {
        ("llm_chat".to_string(), true)
    } else if lower.contains("/completions") {
        ("copilot_completions".to_string(), false)
    } else if lower.contains("/telemetry") {
        ("copilot_telemetry".to_string(), false)
    } else if lower.contains("/models") {
        ("copilot_models".to_string(), false)
    } else if lower.contains("/token") || lower.contains("/oauth") {
        ("copilot_auth".to_string(), false)
    } else {
        ("copilot_api".to_string(), false)
    }
}

include!("proto.rs");
