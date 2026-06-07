use serde_json::{json, Value};
use chrono::Utc;
use uuid::Uuid;

/// Build an OTLP ExportTraceServiceRequest JSON for a single LLM span.
pub fn build_otlp_payload(
    service_name: &str,
    span_name: &str,
    trace_id: &str,
    started_ns: i64,
    ended_ns: i64,
    attributes: Vec<(&str, Value)>,
) -> Value {
    let span_id = hex::encode(&Uuid::new_v4().as_bytes()[..8]);

    let otlp_attrs: Vec<Value> = attributes
        .into_iter()
        .map(|(key, val)| {
            let av = match &val {
                Value::String(s) => json!({"stringValue": s}),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        json!({"intValue": i.to_string()})
                    } else {
                        json!({"doubleValue": n.as_f64().unwrap_or(0.0)})
                    }
                }
                Value::Bool(b) => json!({"boolValue": b}),
                _ => json!({"stringValue": val.to_string()}),
            };
            json!({"key": key, "value": av})
        })
        .collect();

    json!({
        "resourceSpans": [{
            "resource": {
                "attributes": [
                    {"key": "service.name", "value": {"stringValue": service_name}},
                    {"key": "service.namespace", "value": {"stringValue": "ide"}}
                ]
            },
            "scopeSpans": [{
                "scope": {"name": "agent-meter-proxy", "version": env!("CARGO_PKG_VERSION")},
                "spans": [{
                    "traceId": trace_id,
                    "spanId": span_id,
                    "name": span_name,
                    "kind": 3,
                    "startTimeUnixNano": started_ns.to_string(),
                    "endTimeUnixNano": ended_ns.to_string(),
                    "attributes": otlp_attrs,
                    "status": {"code": 1}
                }]
            }]
        }]
    })
}

/// Current timestamp in nanoseconds
pub fn now_ns() -> i64 {
    Utc::now().timestamp_nanos_opt().unwrap_or(0)
}
