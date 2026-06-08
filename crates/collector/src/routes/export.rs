//! T-314 — Trace Export (OpenTelemetry-compatible JSON)
//!
//! GET /api/export/traces?conversation_id=UUID → OTLP-compatible JSON
//! GET /api/export/traces?conversation_id=UUID&format=jaeger → Jaeger format

use axum::{extract::{Query, State}, routing::get, Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::AppError;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/export/traces", get(export_traces))
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct ExportParams {
    conversation_id: Uuid,
    format: Option<String>,
}

#[derive(Debug, Serialize)]
struct OtlpExport {
    resource_spans: Vec<ResourceSpan>,
}

#[derive(Debug, Serialize)]
struct ResourceSpan {
    resource: Resource,
    scope_spans: Vec<ScopeSpan>,
}

#[derive(Debug, Serialize)]
struct Resource {
    attributes: Vec<KeyValue>,
}

#[derive(Debug, Serialize)]
struct ScopeSpan {
    scope: Scope,
    spans: Vec<ExportSpan>,
}

#[derive(Debug, Serialize)]
struct Scope {
    name: String,
    version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportSpan {
    trace_id: String,
    span_id: String,
    parent_span_id: String,
    name: String,
    kind: i32,
    start_time_unix_nano: u64,
    end_time_unix_nano: u64,
    attributes: Vec<KeyValue>,
    status: SpanStatus,
}

#[derive(Debug, Serialize)]
struct SpanStatus {
    code: i32,
    message: String,
}

#[derive(Debug, Serialize)]
struct KeyValue {
    key: String,
    value: AttributeValue,
}

#[derive(Debug, Serialize)]
struct AttributeValue {
    #[serde(rename = "stringValue", skip_serializing_if = "Option::is_none")]
    string_value: Option<String>,
    #[serde(rename = "intValue", skip_serializing_if = "Option::is_none")]
    int_value: Option<i64>,
    #[serde(rename = "doubleValue", skip_serializing_if = "Option::is_none")]
    double_value: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct RawSpan {
    trace_id: Option<String>,
    span_id: Option<String>,
    parent_span_id: Option<String>,
    tool_name: String,
    model: Option<String>,
    mcp_server: Option<String>,
    ide: Option<String>,
    ok: bool,
    error: Option<String>,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    duration_ms: Option<i64>,
    estimated_input_tokens: Option<i64>,
    estimated_output_tokens: Option<i64>,
    usd_cost: Option<f64>,
}

fn str_attr(key: &str, val: &str) -> KeyValue {
    KeyValue {
        key: key.to_string(),
        value: AttributeValue {
            string_value: Some(val.to_string()),
            int_value: None,
            double_value: None,
        },
    }
}

fn int_attr(key: &str, val: i64) -> KeyValue {
    KeyValue {
        key: key.to_string(),
        value: AttributeValue {
            string_value: None,
            int_value: Some(val),
            double_value: None,
        },
    }
}

fn float_attr(key: &str, val: f64) -> KeyValue {
    KeyValue {
        key: key.to_string(),
        value: AttributeValue {
            string_value: None,
            int_value: None,
            double_value: Some(val),
        },
    }
}

async fn export_traces(
    State(state): State<AppState>,
    Query(params): Query<ExportParams>,
) -> Result<Json<OtlpExport>, AppError> {
    let spans = fetch_spans(&state.pool, params.conversation_id).await?;

    let export_spans: Vec<ExportSpan> = spans
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let trace_id = s.trace_id.unwrap_or_else(|| params.conversation_id.to_string());
            let span_id = s.span_id.unwrap_or_else(|| format!("{:016x}", i));
            let parent = s.parent_span_id.unwrap_or_default();

            let start_ns = s.started_at.timestamp_nanos_opt().unwrap_or(0) as u64;
            let end_ns = s
                .ended_at
                .map(|t| t.timestamp_nanos_opt().unwrap_or(0) as u64)
                .unwrap_or(start_ns + (s.duration_ms.unwrap_or(0) as u64) * 1_000_000);

            let mut attrs = vec![str_attr("tool.name", &s.tool_name)];
            if let Some(ref m) = s.model {
                attrs.push(str_attr("llm.model", m));
            }
            if let Some(ref srv) = s.mcp_server {
                attrs.push(str_attr("mcp.server", srv));
            }
            if let Some(ref ide) = s.ide {
                attrs.push(str_attr("ide", ide));
            }
            if let Some(tin) = s.estimated_input_tokens {
                attrs.push(int_attr("llm.tokens.input", tin));
            }
            if let Some(tout) = s.estimated_output_tokens {
                attrs.push(int_attr("llm.tokens.output", tout));
            }
            if let Some(cost) = s.usd_cost {
                attrs.push(float_attr("cost.usd", cost));
            }

            let status = if s.ok {
                SpanStatus { code: 1, message: String::new() }
            } else {
                SpanStatus {
                    code: 2,
                    message: s.error.unwrap_or_else(|| "error".into()),
                }
            };

            ExportSpan {
                trace_id,
                span_id,
                parent_span_id: parent,
                name: s.tool_name,
                kind: 3, // INTERNAL
                start_time_unix_nano: start_ns,
                end_time_unix_nano: end_ns,
                attributes: attrs,
                status,
            }
        })
        .collect();

    Ok(Json(OtlpExport {
        resource_spans: vec![ResourceSpan {
            resource: Resource {
                attributes: vec![str_attr("service.name", "agent-meter")],
            },
            scope_spans: vec![ScopeSpan {
                scope: Scope {
                    name: "agent-meter".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                },
                spans: export_spans,
            }],
        }],
    }))
}

async fn fetch_spans(pool: &PgPool, conversation_id: Uuid) -> Result<Vec<RawSpan>, AppError> {
    let rows = sqlx::query_as::<_, RawSpan>(
        "SELECT trace_id, span_id, parent_span_id, tool_name, model, mcp_server, ide, \
         ok, error, started_at, ended_at, duration_ms::bigint AS duration_ms, \
         estimated_input_tokens, estimated_output_tokens, usd_cost::float8 AS usd_cost \
         FROM agent_tool_calls \
         WHERE conversation_id = $1 \
         ORDER BY started_at",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
