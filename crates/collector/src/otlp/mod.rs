use prost::Message;
use sqlx::PgPool;
use tracing::warn;

use crate::errors::AppError;
use crate::models::event::ToolCallEvent;
use crate::services::event_service;

pub fn handle_trace_request(body: &[u8], pool: &PgPool) -> Result<Vec<serde_json::Value>, AppError> {
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
