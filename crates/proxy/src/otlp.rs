use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

/// Build an OTLP ExportTraceServiceRequest JSON for a single LLM span.
///
/// `http_status`: when `Some(code)` and `code >= 400`, span status is ERROR (2);
/// otherwise OK (1). Matches collector `ok = status.code != 2`.
pub fn build_otlp_payload(
    service_name: &str,
    span_name: &str,
    trace_id: &str,
    started_ns: i64,
    ended_ns: i64,
    attributes: Vec<(&str, Value)>,
    user_agent: Option<&str>,
    http_status: Option<u16>,
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

    let mut resource_attrs = vec![
        json!({"key": "service.name", "value": {"stringValue": service_name}}),
        json!({"key": "service.namespace", "value": {"stringValue": "ide"}}),
    ];
    if let Some(ua) = user_agent.filter(|s| !s.is_empty()) {
        resource_attrs.push(json!({"key": "user_agent", "value": {"stringValue": ua}}));
        resource_attrs.push(json!({"key": "browser.user_agent", "value": {"stringValue": ua}}));
    }

    let status_code = match http_status {
        Some(code) if code >= 400 => 2,
        _ => 1,
    };

    json!({
        "resourceSpans": [{
            "resource": {
                "attributes": resource_attrs
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
                    "status": {"code": status_code}
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
            Some("cursor/0.48.0"),
            None,
        );

        let resource = &payload["resourceSpans"][0]["resource"]["attributes"];
        assert_eq!(resource[0]["key"], "service.name");
        assert_eq!(resource[0]["value"]["stringValue"], "cursor");
        assert_eq!(resource[2]["key"], "user_agent");
        assert_eq!(resource[2]["value"]["stringValue"], "cursor/0.48.0");
        assert_eq!(
            payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["status"]["code"],
            1
        );

        let attrs = &payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["attributes"];
        assert_eq!(attrs[0]["key"], "gen_ai.request.model");
        assert_eq!(attrs[0]["value"]["stringValue"], "gpt-5.4");
        assert_eq!(attrs[1]["key"], "gen_ai.usage.input_tokens");
        assert_eq!(attrs[1]["value"]["intValue"], "42");
        assert_eq!(attrs[2]["key"], "gen_ai.cache.hit");
        assert_eq!(attrs[2]["value"]["boolValue"], true);
        assert_eq!(attrs[3]["key"], "gen_ai.metadata");
        assert!(attrs[3]["value"]["stringValue"]
            .as_str()
            .unwrap()
            .contains("foo"));
    }

    #[test]
    fn build_otlp_payload_marks_error_on_http_4xx() {
        let payload = build_otlp_payload("cursor", "chat", "aa", 1, 2, vec![], None, Some(401));
        assert_eq!(
            payload["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["status"]["code"],
            2
        );
    }

    #[test]
    fn build_otlp_payload_omits_user_agent_when_empty() {
        let payload = build_otlp_payload("copilot", "chat", "aa", 1, 2, vec![], None, None);
        let keys: Vec<_> = payload["resourceSpans"][0]["resource"]["attributes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["key"].as_str().unwrap())
            .collect();
        assert!(!keys.contains(&"user_agent"));
    }
}
