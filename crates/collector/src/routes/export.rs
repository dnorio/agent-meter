//! T-314 — Trace Export (OpenTelemetry-compatible JSON)
//!
//! GET /api/export/traces?conversation_id=UUID → OTLP-compatible JSON
//! GET /api/export/events.csv?from=&to=&model= → CSV of raw events
//! GET /api/export/cost.csv?from=&to= → CSV of daily cost breakdown

use axum::{extract::{Query, State}, http::header, response::IntoResponse, routing::get, Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::app::AppState;
use crate::errors::AppError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/export/traces", get(export_traces))
        .route("/api/export/events.csv", get(export_events_csv))
        .route("/api/export/cost.csv", get(export_cost_csv))
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
         estimated_input_tokens::bigint AS estimated_input_tokens, \
         estimated_output_tokens::bigint AS estimated_output_tokens, \
         usd_cost::float8 AS usd_cost \
         FROM agent_tool_calls \
         WHERE conversation_id = $1::text \
         ORDER BY started_at",
    )
    .bind(conversation_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// ── CSV Exports ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct CsvParams {
    from: Option<String>,
    to: Option<String>,
    model: Option<String>,
    ide: Option<String>,
    agent: Option<String>,
    limit: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct EventCsvRow {
    started_at: DateTime<Utc>,
    tool_name: String,
    model: Option<String>,
    mcp_server: Option<String>,
    ide: Option<String>,
    agent: Option<String>,
    ok: bool,
    duration_ms: Option<i64>,
    estimated_input_tokens: Option<i64>,
    estimated_output_tokens: Option<i64>,
    usd_cost: Option<f64>,
    conversation_id: Option<String>,
    task_id: Option<String>,
}

async fn export_events_csv(
    State(state): State<AppState>,
    Query(p): Query<CsvParams>,
) -> Result<impl IntoResponse, AppError> {
    let to = p.to.as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    let from = p.from.as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| to - Duration::days(30));
    let limit = p.limit.unwrap_or(10000).min(50000);

    let rows: Vec<EventCsvRow> = sqlx::query_as(
        r#"
        SELECT started_at, tool_name, model, mcp_server, ide, agent, ok,
               duration_ms::bigint AS duration_ms,
               estimated_input_tokens::bigint AS estimated_input_tokens,
               estimated_output_tokens::bigint AS estimated_output_tokens,
               usd_cost::float8 AS usd_cost, conversation_id::text AS conversation_id, task_id
        FROM agent_tool_calls
        WHERE started_at >= $1 AND started_at < $2
          AND ($3::text IS NULL OR model = $3)
          AND ($4::text IS NULL OR ide = $4)
          AND ($5::text IS NULL OR agent = $5)
        ORDER BY started_at DESC
        LIMIT $6
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(&p.model)
    .bind(&p.ide)
    .bind(&p.agent)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    let mut csv = String::with_capacity(rows.len() * 200);
    csv.push_str("started_at,tool_name,model,mcp_server,ide,agent,ok,duration_ms,tokens_in,tokens_out,usd_cost,conversation_id,task_id\n");
    for r in &rows {
        use std::fmt::Write;
        let _ = writeln!(
            csv,
            "{},{},{},{},{},{},{},{},{},{},{:.6},{},{}",
            r.started_at.to_rfc3339(),
            csv_escape(&r.tool_name),
            csv_escape(r.model.as_deref().unwrap_or("")),
            csv_escape(r.mcp_server.as_deref().unwrap_or("")),
            csv_escape(r.ide.as_deref().unwrap_or("")),
            csv_escape(r.agent.as_deref().unwrap_or("")),
            r.ok,
            r.duration_ms.unwrap_or(0),
            r.estimated_input_tokens.unwrap_or(0),
            r.estimated_output_tokens.unwrap_or(0),
            r.usd_cost.unwrap_or(0.0),
            csv_escape(r.conversation_id.as_deref().unwrap_or("")),
            csv_escape(r.task_id.as_deref().unwrap_or("")),
        );
    }

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"agent-meter-events.csv\""),
        ],
        csv,
    ))
}

#[derive(sqlx::FromRow)]
struct CostCsvRow {
    day: DateTime<Utc>,
    model: Option<String>,
    events: i64,
    tokens_in: Option<i64>,
    tokens_out: Option<i64>,
    usd_cost: Option<f64>,
}

async fn export_cost_csv(
    State(state): State<AppState>,
    Query(p): Query<CsvParams>,
) -> Result<impl IntoResponse, AppError> {
    let to = p.to.as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    let from = p.from.as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|| to - Duration::days(30));

    let rows: Vec<CostCsvRow> = sqlx::query_as(
        r#"
        SELECT
            date_trunc('day', started_at) AS day,
            model,
            COUNT(*)::bigint AS events,
            SUM(estimated_input_tokens)::bigint AS tokens_in,
            SUM(estimated_output_tokens)::bigint AS tokens_out,
            COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost
        FROM agent_tool_calls
        WHERE started_at >= $1 AND started_at < $2
          AND ($3::text IS NULL OR model = $3)
        GROUP BY 1, model
        ORDER BY 1 ASC, usd_cost DESC
        "#,
    )
    .bind(from)
    .bind(to)
    .bind(&p.model)
    .fetch_all(&state.pool)
    .await?;

    let mut csv = String::with_capacity(rows.len() * 100);
    csv.push_str("date,model,events,tokens_in,tokens_out,usd_cost\n");
    for r in &rows {
        use std::fmt::Write;
        let _ = writeln!(
            csv,
            "{},{},{},{},{},{:.6}",
            r.day.format("%Y-%m-%d"),
            csv_escape(r.model.as_deref().unwrap_or("")),
            r.events,
            r.tokens_in.unwrap_or(0),
            r.tokens_out.unwrap_or(0),
            r.usd_cost.unwrap_or(0.0),
        );
    }

    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"agent-meter-cost.csv\""),
        ],
        csv,
    ))
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
