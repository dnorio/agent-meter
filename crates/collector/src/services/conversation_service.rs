use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::timeline::{ConversationTimeline, TimelineEvent};

// ── Conversation list ──────────────────────────────────────────────────────────

#[derive(sqlx::FromRow, Serialize)]
pub struct ConversationRow {
    pub conversation_id: String,
    pub title: Option<String>,
    pub initial_prompt: Option<String>,
    pub response_preview: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub total_duration_ms: i64,
    pub event_count: i64,
    pub error_count: i64,
    pub total_usd_cost: f64,
    pub agent: Option<String>,
    pub ide: Option<String>,
    pub model: Option<String>,
    pub total_tokens_in: Option<i64>,
    pub total_tokens_out: Option<i64>,
}

pub async fn list_conversations(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    ide_filter: Option<&str>,
) -> Result<Vec<ConversationRow>, AppError> {
    let rows: Vec<ConversationRow> = sqlx::query_as(
        r#"
        SELECT
            conversation_id,
            (SELECT up.user_prompt FROM agent_tool_calls up
             WHERE up.conversation_id = atc.conversation_id
               AND up.user_prompt IS NOT NULL
               AND LENGTH(up.user_prompt) BETWEEN 3 AND 2000
             ORDER BY up.started_at ASC LIMIT 1)                                      AS title,
            LEFT((SELECT up.user_prompt FROM agent_tool_calls up
             WHERE up.conversation_id = atc.conversation_id
               AND up.user_prompt IS NOT NULL
               AND LENGTH(up.user_prompt) BETWEEN 3 AND 2000
             ORDER BY up.started_at ASC LIMIT 1), 300)                                AS initial_prompt,
            LEFT(MAX(tool_result) FILTER (
                WHERE tool_result IS NOT NULL
                  AND LENGTH(tool_result) >= 10
                  AND mcp_server IS NULL
            ), 300)                                                                   AS response_preview,
            MIN(started_at)                                                           AS started_at,
            MAX(ended_at)                                                             AS ended_at,
            COALESCE(SUM(duration_ms), 0)::bigint                                     AS total_duration_ms,
            COUNT(*)::bigint                                                          AS event_count,
            COUNT(*) FILTER (WHERE NOT ok)::bigint                                    AS error_count,
            COALESCE(SUM(usd_cost), 0)::float8                                        AS total_usd_cost,
            MODE() WITHIN GROUP (ORDER BY agent)                                      AS agent,
            MODE() WITHIN GROUP (ORDER BY ide)                                        AS ide,
            MODE() WITHIN GROUP (ORDER BY model)                                      AS model,
            SUM(estimated_input_tokens)::bigint                                       AS total_tokens_in,
            SUM(estimated_output_tokens)::bigint                                      AS total_tokens_out
        FROM agent_tool_calls atc
        WHERE conversation_id IS NOT NULL AND conversation_id <> ''
          AND ($3::text IS NULL OR ide = $3)
        GROUP BY conversation_id
        -- Exclude synthetic/noise single-event conversations:
        -- 1. VS Code title/summary generation (copilotLanguageModelWrapper, title, summarize agents)
        -- 2. Inline completions with no model and no prompt (Eclipse ghost text)
        HAVING NOT (
            COUNT(*) = 1
            AND (
                MAX(agent) IN ('copilotLanguageModelWrapper', 'title', 'summarizeConversationHistory')
                OR BOOL_OR(COALESCE(user_prompt,'') ILIKE 'Please write a brief title%')
                OR BOOL_OR(COALESCE(user_prompt,'') ILIKE 'Summarize the following content%')
                OR (MAX(model) IS NULL AND MAX(user_prompt) IS NULL)
            )
        )
        ORDER BY MIN(started_at) DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .bind(ide_filter)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(sqlx::FromRow)]
struct TimelineRow {
    order: i32,
    tool_name: String,
    mcp_server: Option<String>,
    model: Option<String>,
    duration_ms: i32,
    estimated_input_tokens: Option<i32>,
    estimated_output_tokens: Option<i32>,
    usd_cost: Option<f64>,
    ok: bool,
    started_at: DateTime<Utc>,
    ended_at: DateTime<Utc>,
    user_prompt: Option<String>,
    error: Option<String>,
    tool_arguments: Option<serde_json::Value>,
    tool_result: Option<String>,
    reasoning_tokens: Option<i32>,
    finish_reason: Option<String>,
    request_max_tokens: Option<i32>,
    request_temperature: Option<f64>,
    llm_system: Option<String>,
    trace_id: Option<String>,
    span_id: Option<String>,
    parent_span_id: Option<String>,
    tool_call_id: Option<String>,
}

#[derive(sqlx::FromRow)]
struct SummaryRow {
    user_prompt: Option<String>,
    started_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    total_duration_ms: i64,
    total_tokens_in: Option<i64>,
    total_tokens_out: Option<i64>,
    total_usd_cost: Option<f64>,
    event_count: i64,
    error_count: i64,
}

pub async fn get_conversation_timeline(
    pool: &PgPool,
    conversation_id: &str,
    limit: Option<i64>,
) -> Result<ConversationTimeline, AppError> {
    // Run summary and events queries in parallel
    let summary_fut = sqlx::query_as::<_, SummaryRow>(
        r#"
        SELECT
            (SELECT user_prompt FROM agent_tool_calls
             WHERE conversation_id = $1
               AND user_prompt IS NOT NULL
               AND LENGTH(user_prompt) >= 3
             ORDER BY started_at ASC LIMIT 1) AS user_prompt,
            MIN(started_at) AS started_at,
            MAX(ended_at) AS ended_at,
            SUM(duration_ms)::bigint AS total_duration_ms,
            SUM(estimated_input_tokens)::bigint AS total_tokens_in,
            SUM(estimated_output_tokens)::bigint AS total_tokens_out,
            COALESCE(SUM(usd_cost), 0)::float8 AS total_usd_cost,
            COUNT(*)::bigint AS event_count,
            COUNT(*) FILTER (WHERE NOT ok)::bigint AS error_count
        FROM agent_tool_calls
        WHERE conversation_id = $1
        "#,
    )
    .bind(conversation_id)
    .fetch_optional(pool);

    let events_fut = sqlx::query_as::<_, TimelineRow>(
        r#"
        SELECT
            ROW_NUMBER() OVER (ORDER BY started_at ASC)::integer AS "order",
            tool_name,
            mcp_server,
            model,
            duration_ms,
            estimated_input_tokens,
            estimated_output_tokens,
            usd_cost::float8 AS usd_cost,
            ok,
            started_at,
            ended_at,
            user_prompt,
            error,
            tool_arguments,
            tool_result,
            reasoning_tokens,
            finish_reason,
            request_max_tokens,
            request_temperature,
            llm_system,
            trace_id,
            span_id,
            parent_span_id,
            tool_call_id
        FROM agent_tool_calls
        WHERE conversation_id = $1
        ORDER BY started_at ASC
        LIMIT $2
        "#,
    )
    .bind(conversation_id)
    .bind(limit.unwrap_or(2000))
    .fetch_all(pool);

    let (summary_res, events_res) = tokio::join!(summary_fut, events_fut);
    let summary = summary_res?;
    let events_raw = events_res?;

    let summary = match summary {
        Some(s) => s,
        None => {
            return Ok(ConversationTimeline {
                conversation_id: conversation_id.to_string(),
                title: format!("Conversation {}", conversation_id),
                started_at: chrono::Utc::now(),
                ended_at: chrono::Utc::now(),
                total_duration_ms: 0,
                total_tokens_in: 0,
                total_tokens_out: 0,
                total_usd_cost: 0.0,
                event_count: 0,
                error_count: 0,
                events: vec![],
            });
        }
    };

    let title = summary
        .user_prompt
        .as_ref()
        .map(|p| {
            if p.len() > 80 {
                format!("{}...", &p[..77])
            } else {
                p.clone()
            }
        })
        .unwrap_or_else(|| format!("Conversation {}", conversation_id));

    let events = events_raw
        .into_iter()
        .map(|row| TimelineEvent {
            order: row.order as u32,
            tool_name: row.tool_name,
            mcp_server: row.mcp_server,
            model: row.model,
            duration_ms: row.duration_ms,
            tokens_in: row.estimated_input_tokens,
            tokens_out: row.estimated_output_tokens,
            usd_cost: row.usd_cost.unwrap_or(0.0),
            ok: row.ok,
            started_at: row.started_at,
            ended_at: row.ended_at,
            user_prompt: row.user_prompt,
            error: row.error,
            tool_arguments: row.tool_arguments,
            tool_result: row.tool_result,
            reasoning_tokens: row.reasoning_tokens,
            finish_reason: row.finish_reason,
            request_max_tokens: row.request_max_tokens,
            request_temperature: row.request_temperature,
            llm_system: row.llm_system,
            trace_id: row.trace_id,
            span_id: row.span_id,
            parent_span_id: row.parent_span_id,
            tool_call_id: row.tool_call_id,
        })
        .collect();

    Ok(ConversationTimeline {
        conversation_id: conversation_id.to_string(),
        title,
        started_at: summary.started_at,
        ended_at: summary.ended_at.unwrap_or(summary.started_at),
        total_duration_ms: summary.total_duration_ms,
        total_tokens_in: summary.total_tokens_in.unwrap_or(0),
        total_tokens_out: summary.total_tokens_out.unwrap_or(0),
        total_usd_cost: summary.total_usd_cost.unwrap_or(0.0),
        event_count: summary.event_count,
        error_count: summary.error_count,
        events,
    })
}
