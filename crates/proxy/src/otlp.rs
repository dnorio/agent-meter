use chrono::Utc;
use serde_json::{json, Value};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_otlp_payload_sets_resource_span_and_attribute_types() {
        let payload = build_otlp_payload(
            "cursor",
            "chat gpt-5.4",
            "1234abcd1234abcd1234abcd1234abcd",
            100,
            250,
            vec![
                ("gen_ai.request.model", json!("gpt-5.4")),
                ("gen_ai.usage.input_tokens", json!(42)),
                ("gen_ai.cache.hit", json!(true)),
                ("gen_ai.metadata", json!({"foo": "bar"})),
            ],
        );

        let span = &payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0];
        let attrs = payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["attributes"]
            .as_array()
            .expect("attributes should be an array");

        assert_eq!(
            payload["resourceSpans"][0]["resource"]["attributes"][0]["key"],
            "service.name"
        );
        assert_eq!(
            payload["resourceSpans"][0]["resource"]["attributes"][0]["value"]["stringValue"],
            "cursor"
        );
        assert_eq!(span["traceId"], "1234abcd1234abcd1234abcd1234abcd");
        assert_eq!(span["name"], "chat gpt-5.4");
        assert_eq!(span["startTimeUnixNano"], "100");
        assert_eq!(span["endTimeUnixNano"], "250");
        assert_eq!(span["kind"], 3);

        assert_eq!(attrs[0]["key"], "gen_ai.request.model");
        assert_eq!(attrs[0]["value"]["stringValue"], "gpt-5.4");
        assert_eq!(attrs[1]["value"]["intValue"], "42");
        assert_eq!(attrs[2]["value"]["boolValue"], true);
        assert_eq!(attrs[3]["value"]["stringValue"], "{\"foo\":\"bar\"}");

        let span_id = span["spanId"].as_str().expect("spanId should be a string");
        assert_eq!(span_id.len(), 16);
    }
}
