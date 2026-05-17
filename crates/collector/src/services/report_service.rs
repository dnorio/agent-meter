use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::tool_call::{TopMcpServer, TopTask, TopTool};

pub struct ReportQuery {
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
    pub repo: Option<String>,
    pub ide: Option<String>,
    pub agent: Option<String>,
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
            skill: None,
            limit: Some(20),
        }
    }
}

pub async fn top_tools(pool: &PgPool, q: &ReportQuery) -> Result<Vec<TopTool>, AppError> {
    let rows = sqlx::query_as::<_, TopTool>(
        r#"
        SELECT
            mcp_server,
            tool_name,
            COUNT(*)::bigint as calls,
            SUM(estimated_total_tokens)::bigint as total_estimated_tokens,
            AVG(duration_ms)::float8 as avg_duration_ms,
            COUNT(*) FILTER (WHERE not ok)::bigint as errors,
            AVG(response_bytes)::float8 as avg_response_bytes
        FROM agent_tool_calls
        WHERE ($1::timestamptz IS NULL OR started_at >= $1)
          AND ($2::timestamptz IS NULL OR started_at <= $2)
          AND ($3::text IS NULL OR repo = $3)
          AND ($4::text IS NULL OR ide = $4)
          AND ($5::text IS NULL OR agent = $5)
          AND ($6::text IS NULL OR skill = $6)
        GROUP BY mcp_server, tool_name
        ORDER BY calls DESC
        LIMIT $7
        "#,
    )
    .bind(q.from)
    .bind(q.to)
    .bind(&q.repo)
    .bind(&q.ide)
    .bind(&q.agent)
    .bind(&q.skill)
    .bind(q.limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn top_tasks(pool: &PgPool, q: &ReportQuery) -> Result<Vec<TopTask>, AppError> {
    let rows = sqlx::query_as::<_, TopTask>(
        r#"
        SELECT
            task_id,
            COUNT(*)::bigint as tool_calls,
            SUM(estimated_total_tokens)::bigint as total_estimated_tokens,
            SUM(duration_ms)::bigint as total_duration_ms,
            COUNT(*) FILTER (WHERE not ok)::bigint as errors,
            COUNT(DISTINCT tool_name)::bigint as distinct_tools
        FROM agent_tool_calls
        WHERE task_id IS NOT NULL
          AND ($1::timestamptz IS NULL OR started_at >= $1)
          AND ($2::timestamptz IS NULL OR started_at <= $2)
          AND ($3::text IS NULL OR repo = $3)
          AND ($4::text IS NULL OR ide = $4)
          AND ($5::text IS NULL OR agent = $5)
          AND ($6::text IS NULL OR skill = $6)
        GROUP BY task_id
        ORDER BY tool_calls DESC
        LIMIT $7
        "#,
    )
    .bind(q.from)
    .bind(q.to)
    .bind(&q.repo)
    .bind(&q.ide)
    .bind(&q.agent)
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
          AND ($1::timestamptz IS NULL OR started_at >= $1)
          AND ($2::timestamptz IS NULL OR started_at <= $2)
          AND ($3::text IS NULL OR repo = $3)
          AND ($4::text IS NULL OR ide = $4)
          AND ($5::text IS NULL OR agent = $5)
          AND ($6::text IS NULL OR skill = $6)
        GROUP BY mcp_server
        ORDER BY calls DESC
        LIMIT $7
        "#,
    )
    .bind(q.from)
    .bind(q.to)
    .bind(&q.repo)
    .bind(&q.ide)
    .bind(&q.agent)
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
              AND ($6::text IS NULL OR skill = $6)
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
    .bind(&q.skill)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
