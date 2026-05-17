use prost::Message;
use sqlx::PgPool;
use tracing::warn;

use crate::errors::AppError;
use crate::models::event::ToolCallEvent;
use crate::services::event_service;

/// Dispatcher: selects protobuf or JSON parser based on Content-Type.
pub fn handle_trace_request(
    body: &[u8],
    content_type: Option<&str>,
    pool: &PgPool,
) -> Result<Vec<serde_json::Value>, AppError> {
    if content_type.map(|ct| ct.contains("application/json")).unwrap_or(false) {
        return handle_trace_request_json(body, pool);
    }
    handle_trace_request_proto(body, pool)
}

/// Parse OTLP JSON (format sent by VS Code / OpenTelemetry JS SDK).
fn handle_trace_request_json(body: &[u8], pool: &PgPool) -> Result<Vec<serde_json::Value>, AppError> {
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

        let scope_spans = rs.get("scopeSpans").and_then(|v| v.as_array());
        let Some(scope_spans) = scope_spans else { continue };

        for ss in scope_spans {
            let spans = ss.get("spans").and_then(|v| v.as_array());
            let Some(spans) = spans else { continue };

            for span in spans {
                let span_name = span.get("name").and_then(|v| v.as_str()).unwrap_or("");
                match span_name {
                    "execute_tool" => {
                        match map_tool_call_json(span, service_name.as_deref(), session_id.as_deref()) {
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
                            Err(e) => warn!("failed to map JSON OTLP tool_call span: {e}"),
                        }
                    }
                    "invoke_agent" | "chat" => {}
                    _ => {
                        warn!("unknown OTLP span name (json): {}", span_name);
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
) -> Result<ToolCallEvent, &'static str> {
    let attrs = span.get("attributes").and_then(|v| v.as_array());
    let attrs_slice: &[serde_json::Value] = attrs.map(|a| a.as_slice()).unwrap_or(&[]);

    let tool_name = json_attr_str(attrs_slice, "gen_ai.tool.name")
        .or_else(|| span.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .ok_or("missing tool name")?;

    let agent_name = json_attr_str(attrs_slice, "gen_ai.agent.name")
        .or_else(|| service_name.map(|s| s.to_string()));

    let mcp_server = json_attr_str(attrs_slice, "mcp.server");

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

    let input_tokens = json_attr_int(attrs_slice, "gen_ai.usage.input_tokens");
    let output_tokens = json_attr_int(attrs_slice, "gen_ai.usage.output_tokens");
    let model = json_attr_str(attrs_slice, "gen_ai.response.model");
    let response_bytes = json_attr_int(attrs_slice, "gen_ai.response.bytes");

    let mut metadata = serde_json::json!({});
    if let Some(svc) = service_name { metadata["service_name"] = serde_json::json!(svc); }
    if let Some(sid) = session_id { metadata["session_id"] = serde_json::json!(sid); }
    if let Some(m) = &model { metadata["model"] = serde_json::json!(m); }
    if let Some(a) = &agent_name { metadata["agent"] = serde_json::json!(a); }

    Ok(ToolCallEvent {
        event_id: uuid::Uuid::new_v4(),
        task_id: None,
        repo: None,
        branch: None,
        ide: Some("copilot-vscode".into()),
        agent: agent_name,
        skill: None,
        mcp_server,
        tool_name,
        started_at: start,
        ended_at: end,
        ok,
        error,
        request_bytes: None,
        response_bytes: response_bytes.map(|b| b as i32),
        estimated_input_tokens: input_tokens.map(|t| t as i32),
        estimated_output_tokens: output_tokens.map(|t| t as i32),
        request_sha256: None,
        response_sha256: None,
        metadata: Some(metadata),
    })
}

fn json_attr_str(attrs: &[serde_json::Value], key: &str) -> Option<String> {
    attrs.iter()
        .find(|kv| kv.get("key").and_then(|k| k.as_str()) == Some(key))
        .and_then(|kv| kv.pointer("/value/stringValue").and_then(|v| v.as_str()).map(|s| s.to_string()))
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

fn handle_trace_request_proto(body: &[u8], pool: &PgPool) -> Result<Vec<serde_json::Value>, AppError> {
    let req = ExportTraceServiceRequest::decode(body)
        .map_err(|e| AppError::Validation(format!("failed to decode OTLP protobuf: {e}")))?;

    let mut results = Vec::new();

    for rs in &req.resource_spans {
        let resource_attrs = rs.resource.as_ref().map(|r| &r.attributes[..]).unwrap_or(&[]);
        let service_name = get_attr_str(resource_attrs, "service.name");
        let session_id = get_attr_str(resource_attrs, "session.id");

        for ss in &rs.scope_spans {
            for span in &ss.spans {
                match span.name.as_str() {
                    "execute_tool" => {
                        match map_tool_call(span, resource_attrs, service_name.as_deref(), session_id.as_deref()) {
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
                            Err(e) => warn!("failed to map OTLP tool_call span: {e}"),
                        }
                    }
                    "invoke_agent" | "chat" => {
                        // these are handled in-band by execute_tool's parent linkage
                        // could later create tasks from invoke_agent spans
                    }
                    _ => {
                        warn!("unknown OTLP span name: {}", span.name);
                    }
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
) -> Result<ToolCallEvent, &'static str> {
    let tool_name = get_attr_str(&span.attributes, "gen_ai.tool.name")
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

    let input_tokens = get_attr_int(&span.attributes, "gen_ai.usage.input_tokens");
    let output_tokens = get_attr_int(&span.attributes, "gen_ai.usage.output_tokens");
    let model = get_attr_str(&span.attributes, "gen_ai.response.model");

    let mut metadata = serde_json::json!({});
    if let Some(svc) = service_name {
        metadata["service_name"] = serde_json::json!(svc);
    }
    if let Some(sid) = session_id {
        metadata["session_id"] = serde_json::json!(sid);
    }
    if let Some(m) = model {
        metadata["model"] = serde_json::json!(m);
    }
    if let Some(agent) = &agent_name {
        metadata["agent"] = serde_json::json!(agent);
    }

    let event = ToolCallEvent {
        event_id: uuid::Uuid::new_v4(),
        task_id: None,
        repo: None,
        branch: None,
        ide: Some("copilot-vscode".into()),
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
    };

    Ok(event)
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
