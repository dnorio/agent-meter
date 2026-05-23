use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::tool_call::{EventFeedRow, TopMcpServer, TopTask, TopTool};

pub struct ReportQuery {
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
    pub repo: Option<String>,
    pub ide: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub skill: Option<String>,
    pub limit: Option<i64>,
}

impl Default for ReportQuery {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            repo: None,
            ide: None,
            agent: None,
            model: None,
            skill: None,
            limit: Some(20),
        }
    }
}

pub struct EventQuery {
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
    pub ide: Option<String>,
    pub agent: Option<String>,
    pub model: Option<String>,
    pub conversation_id: Option<String>,
    pub before_started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub before_event_id: Option<uuid::Uuid>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for EventQuery {
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            ide: None,
            agent: None,
            model: None,
            conversation_id: None,
            before_started_at: None,
            before_event_id: None,
            limit: 50,
            offset: 0,
        }
    }
}

pub async fn top_tools(pool: &PgPool, q: &ReportQuery) -> Result<Vec<TopTool>, AppError> {
    let rows = sqlx::query_as::<_, TopTool>(
        r#"
        WITH filtered AS (
            SELECT
                mcp_server,
                tool_name,
                model,
                estimated_total_tokens,
                duration_ms,
                ok,
                response_bytes,
                cached_tokens,
                estimated_input_tokens,
                estimated_output_tokens
            FROM agent_tool_calls
            WHERE ($1::timestamptz IS NULL OR started_at >= $1)
              AND ($2::timestamptz IS NULL OR started_at <= $2)
              AND ($3::text IS NULL OR repo = $3)
              AND ($4::text IS NULL OR ide = $4)
              AND ($5::text IS NULL OR agent = $5)
              AND ($6::text IS NULL OR model = $6)
              AND ($7::text IS NULL OR skill = $7)
        ),
        agg AS (
            SELECT
                mcp_server,
                tool_name,
                COUNT(*)::bigint AS calls,
                SUM(estimated_total_tokens)::bigint AS total_estimated_tokens,
                AVG(duration_ms)::float8 AS avg_duration_ms,
                COUNT(*) FILTER (WHERE NOT ok)::bigint AS errors,
                AVG(response_bytes)::float8 AS avg_response_bytes,
                SUM(cached_tokens)::bigint AS cached_tokens_total,
                AVG(estimated_input_tokens)::float8 AS avg_input_tokens,
                AVG(estimated_output_tokens)::float8 AS avg_output_tokens
            FROM filtered
            GROUP BY mcp_server, tool_name
        ),
        top_models AS (
            SELECT mcp_server, tool_name, model
            FROM (
                SELECT
                    mcp_server,
                    tool_name,
                    model,
                    ROW_NUMBER() OVER (
                        PARTITION BY mcp_server, tool_name
                        ORDER BY COUNT(*) DESC, model ASC
                    ) AS rn
                FROM filtered
                WHERE model IS NOT NULL
                GROUP BY mcp_server, tool_name, model
            ) ranked
            WHERE rn = 1
        )
        SELECT
            agg.mcp_server,
            agg.tool_name,
            agg.calls,
            agg.total_estimated_tokens,
            agg.avg_duration_ms,
            agg.errors,
            agg.avg_response_bytes,
            top_models.model AS top_model,
            agg.cached_tokens_total,
            agg.avg_input_tokens,
            agg.avg_output_tokens
        FROM agg
        LEFT JOIN top_models
          ON top_models.mcp_server IS NOT DISTINCT FROM agg.mcp_server
         AND top_models.tool_name = agg.tool_name
        ORDER BY calls DESC
        LIMIT $8
        "#,
    )
    .bind(q.from)
    .bind(q.to)
    .bind(&q.repo)
    .bind(&q.ide)
    .bind(&q.agent)
    .bind(&q.model)
    .bind(&q.skill)
    .bind(q.limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn events_feed(pool: &PgPool, q: &EventQuery) -> Result<Vec<EventFeedRow>, AppError> {
    let rows = sqlx::query_as::<_, EventFeedRow>(
        r#"
        SELECT
            event_id, tool_name, model, started_at, duration_ms, ok,
            estimated_input_tokens, estimated_output_tokens, cached_tokens,
            agent, ide, mcp_server, conversation_id, client_ip, user_prompt
        FROM agent_tool_calls
        WHERE ($1::timestamptz IS NULL OR started_at >= $1)
          AND ($2::timestamptz IS NULL OR started_at <= $2)
          AND ($3::text IS NULL OR ide = $3)
          AND ($4::text IS NULL OR agent = $4)
          AND ($5::text IS NULL OR model = $5)
          AND ($6::text IS NULL OR conversation_id = $6)
                    AND (
                                $7::timestamptz IS NULL
                                OR started_at < $7
                                OR (
                                        started_at = $7
                                        AND event_id < COALESCE($8::uuid, 'ffffffff-ffff-ffff-ffff-ffffffffffff'::uuid)
                                )
                    )
                ORDER BY started_at DESC, event_id DESC
                LIMIT $9 OFFSET $10
        "#,
    )
    .bind(q.from)
    .bind(q.to)
    .bind(&q.ide)
    .bind(&q.agent)
    .bind(&q.model)
    .bind(&q.conversation_id)
        .bind(q.before_started_at)
        .bind(q.before_event_id)
    .bind(q.limit)
    .bind(q.offset)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn top_tasks(pool: &PgPool, q: &ReportQuery) -> Result<Vec<TopTask>, AppError> {
    let rows = sqlx::query_as::<_, TopTask>(
        r#"
        SELECT
            conversation_id as task_id,
            COUNT(*)::bigint as tool_calls,
            SUM(estimated_total_tokens)::bigint as total_estimated_tokens,
            SUM(duration_ms)::bigint as total_duration_ms,
            COUNT(*) FILTER (WHERE not ok)::bigint as errors,
            COUNT(DISTINCT tool_name)::bigint as distinct_tools
        FROM agent_tool_calls
        WHERE conversation_id IS NOT NULL
          AND ($1::timestamptz IS NULL OR started_at >= $1)
          AND ($2::timestamptz IS NULL OR started_at <= $2)
          AND ($3::text IS NULL OR repo = $3)
          AND ($4::text IS NULL OR ide = $4)
          AND ($5::text IS NULL OR agent = $5)
                    AND ($6::text IS NULL OR model = $6)
                    AND ($7::text IS NULL OR skill = $7)
        GROUP BY conversation_id
        ORDER BY tool_calls DESC
                LIMIT $8
        "#,
    )
    .bind(q.from)
    .bind(q.to)
    .bind(&q.repo)
    .bind(&q.ide)
    .bind(&q.agent)
        .bind(&q.model)
    .bind(&q.skill)
    .bind(q.limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn top_mcp_servers(pool: &PgPool, q: &ReportQuery) -> Result<Vec<TopMcpServer>, AppError> {
    let rows = sqlx::query_as::<_, TopMcpServer>(
        r#"
        SELECT
            mcp_server,
            COUNT(*)::bigint as calls,
            SUM(estimated_total_tokens)::bigint as total_estimated_tokens,
            AVG(response_bytes)::float8 as avg_response_bytes,
            (COUNT(*) FILTER (WHERE not ok)::float8 / NULLIF(COUNT(*)::float8, 0)) as error_rate
        FROM agent_tool_calls
        WHERE mcp_server IS NOT NULL
          AND tool_name != 'llm_chat'
          AND ($1::timestamptz IS NULL OR started_at >= $1)
          AND ($2::timestamptz IS NULL OR started_at <= $2)
          AND ($3::text IS NULL OR repo = $3)
          AND ($4::text IS NULL OR ide = $4)
          AND ($5::text IS NULL OR agent = $5)
                    AND ($6::text IS NULL OR model = $6)
                    AND ($7::text IS NULL OR skill = $7)
        GROUP BY mcp_server
        ORDER BY calls DESC
                LIMIT $8
        "#,
    )
    .bind(q.from)
    .bind(q.to)
    .bind(&q.repo)
    .bind(&q.ide)
    .bind(&q.agent)
        .bind(&q.model)
    .bind(&q.skill)
    .bind(q.limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct CallBucket {
    pub bucket: chrono::DateTime<chrono::Utc>,
    pub calls: i64,
    pub errors: i64,
}

pub async fn calls_over_time(
    pool: &PgPool,
    q: &ReportQuery,
    bucket: &str,
) -> Result<Vec<CallBucket>, AppError> {
    let interval = match bucket {
        "hour" => "hour",
        "day" => "day",
        "minute" => "minute",
        _ => "hour",
    };

    let rows = sqlx::query_as::<_, CallBucket>(
        &format!(
            r#"
            SELECT
                date_trunc('{interval}', started_at) as bucket,
                COUNT(*)::bigint as calls,
                COUNT(*) FILTER (WHERE not ok)::bigint as errors
            FROM agent_tool_calls
            WHERE ($1::timestamptz IS NULL OR started_at >= $1)
              AND ($2::timestamptz IS NULL OR started_at <= $2)
              AND ($3::text IS NULL OR repo = $3)
              AND ($4::text IS NULL OR ide = $4)
              AND ($5::text IS NULL OR agent = $5)
                            AND ($6::text IS NULL OR model = $6)
                            AND ($7::text IS NULL OR skill = $7)
            GROUP BY bucket
            ORDER BY bucket ASC
            LIMIT 500
            "#,
            interval = interval
        ),
    )
    .bind(q.from)
    .bind(q.to)
    .bind(&q.repo)
    .bind(&q.ide)
    .bind(&q.agent)
    .bind(&q.model)
    .bind(&q.skill)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
