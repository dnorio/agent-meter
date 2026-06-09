//! PostgreSQL implementation of the Database trait.

use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

use crate::models::*;
use crate::params::*;
use crate::{Database, DbError, DbResult};

/// PostgreSQL-backed database.
#[derive(Clone)]
pub struct PostgresDb {
    pool: PgPool,
}

impl PostgresDb {
    /// Connect to PostgreSQL.
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await
            .map_err(|e| DbError::Internal(format!("pg connect: {e}")))?;

        sqlx::query("SELECT 1")
            .execute(&pool)
            .await
            .map_err(|e| DbError::Internal(format!("pg ping: {e}")))?;

        tracing::info!("connected to PostgreSQL");
        Ok(Self { pool })
    }

    /// Create from an existing pool (useful for tests / existing code migration).
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the underlying pool (escape hatch during migration).
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

// ── Internal row types (with sqlx::FromRow) ─────────────────────────────────

#[derive(sqlx::FromRow)]
struct PgToolCallRow {
    id: i64,
    event_id: uuid::Uuid,
    task_id: Option<String>,
    repo: Option<String>,
    branch: Option<String>,
    ide: Option<String>,
    agent: Option<String>,
    skill: Option<String>,
    mcp_server: Option<String>,
    tool_name: String,
    started_at: chrono::DateTime<chrono::Utc>,
    ended_at: chrono::DateTime<chrono::Utc>,
    duration_ms: i32,
    ok: bool,
    error: Option<String>,
    request_bytes: Option<i32>,
    response_bytes: Option<i32>,
    estimated_input_tokens: Option<i32>,
    estimated_output_tokens: Option<i32>,
    estimated_total_tokens: Option<i32>,
    request_sha256: Option<String>,
    response_sha256: Option<String>,
    metadata: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    model: Option<String>,
    cached_tokens: Option<i32>,
    conversation_id: Option<String>,
    client_ip: Option<String>,
    user_agent: Option<String>,
    user_prompt: Option<String>,
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
    usd_cost: Option<f64>,
    billing_model: String,
}

impl From<PgToolCallRow> for ToolCallRow {
    fn from(r: PgToolCallRow) -> Self {
        Self {
            id: r.id,
            event_id: r.event_id,
            task_id: r.task_id,
            repo: r.repo,
            branch: r.branch,
            ide: r.ide,
            agent: r.agent,
            skill: r.skill,
            mcp_server: r.mcp_server,
            tool_name: r.tool_name,
            started_at: r.started_at,
            ended_at: r.ended_at,
            duration_ms: r.duration_ms,
            ok: r.ok,
            error: r.error,
            request_bytes: r.request_bytes,
            response_bytes: r.response_bytes,
            estimated_input_tokens: r.estimated_input_tokens,
            estimated_output_tokens: r.estimated_output_tokens,
            estimated_total_tokens: r.estimated_total_tokens,
            request_sha256: r.request_sha256,
            response_sha256: r.response_sha256,
            metadata: r.metadata,
            created_at: r.created_at,
            model: r.model,
            cached_tokens: r.cached_tokens,
            conversation_id: r.conversation_id,
            client_ip: r.client_ip,
            user_agent: r.user_agent,
            user_prompt: r.user_prompt,
            tool_arguments: r.tool_arguments,
            tool_result: r.tool_result,
            reasoning_tokens: r.reasoning_tokens,
            finish_reason: r.finish_reason,
            request_max_tokens: r.request_max_tokens,
            request_temperature: r.request_temperature,
            llm_system: r.llm_system,
            trace_id: r.trace_id,
            span_id: r.span_id,
            parent_span_id: r.parent_span_id,
            tool_call_id: r.tool_call_id,
            usd_cost: r.usd_cost,
            billing_model: r.billing_model,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PgEventFeedRow {
    event_id: uuid::Uuid,
    tool_name: String,
    model: Option<String>,
    started_at: chrono::DateTime<chrono::Utc>,
    duration_ms: i32,
    ok: bool,
    estimated_input_tokens: Option<i32>,
    estimated_output_tokens: Option<i32>,
    cached_tokens: Option<i32>,
    agent: Option<String>,
    ide: Option<String>,
    mcp_server: Option<String>,
    conversation_id: Option<String>,
    client_ip: Option<String>,
    user_prompt: Option<String>,
    tool_arguments: Option<serde_json::Value>,
    tool_result: Option<String>,
}

impl From<PgEventFeedRow> for EventFeedRow {
    fn from(r: PgEventFeedRow) -> Self {
        Self {
            event_id: r.event_id,
            tool_name: r.tool_name,
            model: r.model,
            started_at: r.started_at,
            duration_ms: r.duration_ms,
            ok: r.ok,
            estimated_input_tokens: r.estimated_input_tokens,
            estimated_output_tokens: r.estimated_output_tokens,
            cached_tokens: r.cached_tokens,
            agent: r.agent,
            ide: r.ide,
            mcp_server: r.mcp_server,
            conversation_id: r.conversation_id,
            client_ip: r.client_ip,
            user_prompt: r.user_prompt,
            tool_arguments: r.tool_arguments,
            tool_result: r.tool_result,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PgTopToolRow {
    mcp_server: Option<String>,
    tool_name: String,
    calls: i64,
    total_estimated_tokens: Option<i64>,
    avg_duration_ms: Option<f64>,
    errors: i64,
    avg_response_bytes: Option<f64>,
    top_model: Option<String>,
    cached_tokens_total: Option<i64>,
    avg_input_tokens: Option<f64>,
    avg_output_tokens: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct PgTopAgentRow {
    agent: String,
    calls: i64,
    total_tokens: Option<i64>,
    total_usd_cost: Option<f64>,
    errors: i64,
    conversations: i64,
}

#[derive(sqlx::FromRow)]
struct PgTopMcpServerRow {
    mcp_server: String,
    calls: i64,
    total_estimated_tokens: Option<i64>,
    avg_response_bytes: Option<f64>,
    error_rate: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct PgIdeBreakdownRow {
    ide: String,
    calls: i64,
    total_estimated_tokens: Option<i64>,
    errors: i64,
    llm_calls: i64,
    tool_calls_count: i64,
}

#[derive(sqlx::FromRow)]
struct PgErrorPatternRow {
    error: String,
    occurrences: i64,
    tool_name: Option<String>,
    model: Option<String>,
    last_seen: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow)]
struct PgCostBucketRow {
    bucket: chrono::DateTime<chrono::Utc>,
    total_usd: Option<f64>,
    calls: i64,
}

#[derive(sqlx::FromRow)]
struct PgConversationRow {
    conversation_id: String,
    title: Option<String>,
    initial_prompt: Option<String>,
    response_preview: Option<String>,
    started_at: chrono::DateTime<chrono::Utc>,
    ended_at: Option<chrono::DateTime<chrono::Utc>>,
    total_duration_ms: i64,
    event_count: i64,
    error_count: i64,
    total_usd_cost: f64,
    agent: Option<String>,
    ide: Option<String>,
    model: Option<String>,
    total_tokens_in: Option<i64>,
    total_tokens_out: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct PgOrgRow {
    id: uuid::Uuid,
    slug: String,
    name: String,
    plan: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(sqlx::FromRow)]
struct PgApiKeyRow {
    id: uuid::Uuid,
    org_id: uuid::Uuid,
    key_prefix: String,
    name: String,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(sqlx::FromRow)]
struct PgApiKeyMetaRow {
    id: uuid::Uuid,
    org_id: uuid::Uuid,
    key_hash: String,
}

#[derive(sqlx::FromRow)]
struct PgSearchResultRow {
    conversation_id: String,
    user_prompt: Option<String>,
    model: Option<String>,
    agent: Option<String>,
    tool_name: Option<String>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    match_field: String,
}

// ── Trait implementation ────────────────────────────────────────────────────

#[async_trait]
impl Database for PostgresDb {
    async fn insert_tool_call(&self, e: &InsertToolCall) -> DbResult<ToolCallRow> {
        let row = sqlx::query_as::<_, PgToolCallRow>(
            r#"
            INSERT INTO agent_tool_calls (
                event_id, task_id, repo, branch, ide, agent, skill,
                mcp_server, tool_name, started_at, ended_at, duration_ms,
                ok, error, request_bytes, response_bytes,
                estimated_input_tokens, estimated_output_tokens, estimated_total_tokens,
                request_sha256, response_sha256, metadata,
                model, cached_tokens, conversation_id, client_ip, user_agent, user_prompt,
                tool_arguments, tool_result,
                reasoning_tokens, finish_reason, request_max_tokens, request_temperature,
                llm_system, trace_id, span_id, parent_span_id, tool_call_id,
                usd_cost, billing_model
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16, $17, $18, $19, $20, $21, $22,
                $23, $24, $25, $26, $27, $28,
                $29, $30,
                $31, $32, $33, $34, $35, $36, $37, $38, $39,
                compute_event_usd($23, $17, $18, $24),
                CASE WHEN $5 ILIKE '%copilot%' OR $5 ILIKE '%vscode%' THEN 'copilot_credit'
                     WHEN $5 ILIKE '%cursor%' THEN 'cursor_usage'
                     ELSE 'token' END
            )
            RETURNING *
            "#,
        )
        .bind(e.event_id)
        .bind(&e.task_id)
        .bind(&e.repo)
        .bind(&e.branch)
        .bind(&e.ide)
        .bind(&e.agent)
        .bind(&e.skill)
        .bind(&e.mcp_server)
        .bind(&e.tool_name)
        .bind(e.started_at)
        .bind(e.ended_at)
        .bind(e.duration_ms)
        .bind(e.ok)
        .bind(&e.error)
        .bind(e.request_bytes)
        .bind(e.response_bytes)
        .bind(e.estimated_input_tokens)
        .bind(e.estimated_output_tokens)
        .bind(e.estimated_total_tokens)
        .bind(&e.request_sha256)
        .bind(&e.response_sha256)
        .bind(&e.metadata)
        .bind(&e.model)
        .bind(e.cached_tokens)
        .bind(&e.conversation_id)
        .bind(&e.client_ip)
        .bind(&e.user_agent)
        .bind(&e.user_prompt)
        .bind(&e.tool_arguments)
        .bind(&e.tool_result)
        .bind(e.reasoning_tokens)
        .bind(&e.finish_reason)
        .bind(e.request_max_tokens)
        .bind(e.request_temperature)
        .bind(&e.llm_system)
        .bind(&e.trace_id)
        .bind(&e.span_id)
        .bind(&e.parent_span_id)
        .bind(&e.tool_call_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    async fn query_events(&self, q: &EventQuery) -> DbResult<Vec<EventFeedRow>> {
        let rows = sqlx::query_as::<_, PgEventFeedRow>(
            r#"
            SELECT
                event_id, tool_name, model, started_at, duration_ms, ok,
                estimated_input_tokens, estimated_output_tokens, cached_tokens,
                agent, ide, mcp_server, conversation_id, client_ip, user_prompt,
                tool_arguments, tool_result
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
                  OR (started_at = $7 AND event_id < COALESCE($8::uuid, 'ffffffff-ffff-ffff-ffff-ffffffffffff'::uuid))
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
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn top_tools(&self, q: &ReportQuery) -> DbResult<Vec<TopToolRow>> {
        let rows = sqlx::query_as::<_, PgTopToolRow>(
            r#"
            WITH filtered AS (
                SELECT mcp_server, tool_name, model, estimated_total_tokens,
                       duration_ms, ok, response_bytes, cached_tokens,
                       estimated_input_tokens, estimated_output_tokens
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
                SELECT mcp_server, tool_name,
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
                    SELECT mcp_server, tool_name, model,
                        ROW_NUMBER() OVER (PARTITION BY mcp_server, tool_name ORDER BY COUNT(*) DESC, model ASC) AS rn
                    FROM filtered WHERE model IS NOT NULL
                    GROUP BY mcp_server, tool_name, model
                ) ranked WHERE rn = 1
            )
            SELECT agg.mcp_server, agg.tool_name, agg.calls, agg.total_estimated_tokens,
                   agg.avg_duration_ms, agg.errors, agg.avg_response_bytes,
                   top_models.model AS top_model, agg.cached_tokens_total,
                   agg.avg_input_tokens, agg.avg_output_tokens
            FROM agg
            LEFT JOIN top_models ON top_models.mcp_server IS NOT DISTINCT FROM agg.mcp_server
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
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| TopToolRow {
            mcp_server: r.mcp_server,
            tool_name: r.tool_name,
            calls: r.calls,
            total_estimated_tokens: r.total_estimated_tokens,
            avg_duration_ms: r.avg_duration_ms,
            errors: r.errors,
            avg_response_bytes: r.avg_response_bytes,
            top_model: r.top_model,
            cached_tokens_total: r.cached_tokens_total,
            avg_input_tokens: r.avg_input_tokens,
            avg_output_tokens: r.avg_output_tokens,
        }).collect())
    }

    async fn top_agents(&self, q: &ReportQuery) -> DbResult<Vec<TopAgentRow>> {
        let rows = sqlx::query_as::<_, PgTopAgentRow>(
            r#"
            SELECT
                agent,
                COUNT(*)::bigint AS calls,
                SUM(estimated_total_tokens)::bigint AS total_tokens,
                SUM(usd_cost)::float8 AS total_usd_cost,
                COUNT(*) FILTER (WHERE NOT ok)::bigint AS errors,
                COUNT(DISTINCT conversation_id)::bigint AS conversations
            FROM agent_tool_calls
            WHERE agent IS NOT NULL
              AND ($1::timestamptz IS NULL OR started_at >= $1)
              AND ($2::timestamptz IS NULL OR started_at <= $2)
              AND ($3::text IS NULL OR repo = $3)
              AND ($4::text IS NULL OR ide = $4)
              AND ($5::text IS NULL OR model = $5)
            GROUP BY agent
            ORDER BY calls DESC
            LIMIT $6
            "#,
        )
        .bind(q.from)
        .bind(q.to)
        .bind(&q.repo)
        .bind(&q.ide)
        .bind(&q.model)
        .bind(q.limit.unwrap_or(20))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| TopAgentRow {
            agent: r.agent, calls: r.calls, total_tokens: r.total_tokens,
            total_usd_cost: r.total_usd_cost, errors: r.errors, conversations: r.conversations,
        }).collect())
    }

    async fn top_mcp_servers(&self, q: &ReportQuery) -> DbResult<Vec<TopMcpServerRow>> {
        let rows = sqlx::query_as::<_, PgTopMcpServerRow>(
            r#"
            SELECT
                mcp_server,
                COUNT(*)::bigint AS calls,
                SUM(estimated_total_tokens)::bigint AS total_estimated_tokens,
                AVG(response_bytes)::float8 AS avg_response_bytes,
                (COUNT(*) FILTER (WHERE NOT ok)::float8 / NULLIF(COUNT(*), 0)::float8) AS error_rate
            FROM agent_tool_calls
            WHERE mcp_server IS NOT NULL AND mcp_server <> ''
              AND tool_name <> 'llm_chat'
              AND ($1::timestamptz IS NULL OR started_at >= $1)
              AND ($2::timestamptz IS NULL OR started_at <= $2)
              AND ($3::text IS NULL OR repo = $3)
              AND ($4::text IS NULL OR ide = $4)
            GROUP BY mcp_server
            ORDER BY calls DESC
            LIMIT $5
            "#,
        )
        .bind(q.from)
        .bind(q.to)
        .bind(&q.repo)
        .bind(&q.ide)
        .bind(q.limit.unwrap_or(20))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| TopMcpServerRow {
            mcp_server: r.mcp_server, calls: r.calls,
            total_estimated_tokens: r.total_estimated_tokens,
            avg_response_bytes: r.avg_response_bytes, error_rate: r.error_rate,
        }).collect())
    }

    async fn ide_breakdown(&self, q: &ReportQuery) -> DbResult<Vec<IdeBreakdownRow>> {
        let rows = sqlx::query_as::<_, PgIdeBreakdownRow>(
            r#"
            SELECT
                COALESCE(ide, 'unknown') AS ide,
                COUNT(*)::bigint AS calls,
                SUM(estimated_total_tokens)::bigint AS total_estimated_tokens,
                COUNT(*) FILTER (WHERE NOT ok)::bigint AS errors,
                COUNT(*) FILTER (WHERE mcp_server IS NULL AND tool_name = 'llm_chat')::bigint AS llm_calls,
                COUNT(*) FILTER (WHERE mcp_server IS NOT NULL OR tool_name <> 'llm_chat')::bigint AS tool_calls_count
            FROM agent_tool_calls
            WHERE ($1::timestamptz IS NULL OR started_at >= $1)
              AND ($2::timestamptz IS NULL OR started_at <= $2)
              AND ($3::text IS NULL OR repo = $3)
            GROUP BY ide
            ORDER BY calls DESC
            "#,
        )
        .bind(q.from)
        .bind(q.to)
        .bind(&q.repo)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| IdeBreakdownRow {
            ide: r.ide, calls: r.calls, total_estimated_tokens: r.total_estimated_tokens,
            errors: r.errors, llm_calls: r.llm_calls, tool_calls_count: r.tool_calls_count,
        }).collect())
    }

    async fn error_patterns(&self, q: &ReportQuery) -> DbResult<Vec<ErrorPatternRow>> {
        let rows = sqlx::query_as::<_, PgErrorPatternRow>(
            r#"
            SELECT
                error,
                COUNT(*)::bigint AS occurrences,
                MODE() WITHIN GROUP (ORDER BY tool_name) AS tool_name,
                MODE() WITHIN GROUP (ORDER BY model) AS model,
                MAX(started_at) AS last_seen
            FROM agent_tool_calls
            WHERE NOT ok AND error IS NOT NULL
              AND ($1::timestamptz IS NULL OR started_at >= $1)
              AND ($2::timestamptz IS NULL OR started_at <= $2)
              AND ($3::text IS NULL OR repo = $3)
              AND ($4::text IS NULL OR ide = $4)
            GROUP BY error
            ORDER BY occurrences DESC
            LIMIT $5
            "#,
        )
        .bind(q.from)
        .bind(q.to)
        .bind(&q.repo)
        .bind(&q.ide)
        .bind(q.limit.unwrap_or(20))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| ErrorPatternRow {
            error: r.error, occurrences: r.occurrences, tool_name: r.tool_name,
            model: r.model, last_seen: r.last_seen,
        }).collect())
    }

    async fn cost_over_time(&self, q: &ReportQuery) -> DbResult<Vec<CostBucketRow>> {
        let rows = sqlx::query_as::<_, PgCostBucketRow>(
            r#"
            SELECT
                date_trunc('hour', started_at) AS bucket,
                SUM(usd_cost)::float8 AS total_usd,
                COUNT(*)::bigint AS calls
            FROM agent_tool_calls
            WHERE ($1::timestamptz IS NULL OR started_at >= $1)
              AND ($2::timestamptz IS NULL OR started_at <= $2)
              AND ($3::text IS NULL OR repo = $3)
              AND ($4::text IS NULL OR ide = $4)
            GROUP BY bucket
            ORDER BY bucket
            "#,
        )
        .bind(q.from)
        .bind(q.to)
        .bind(&q.repo)
        .bind(&q.ide)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| CostBucketRow {
            bucket: r.bucket, total_usd: r.total_usd, calls: r.calls,
        }).collect())
    }

    async fn top_tasks(&self, q: &ReportQuery) -> DbResult<Vec<TopTaskRow>> {
        #[derive(sqlx::FromRow)]
        struct Row {
            task_id: String,
            tool_calls: i64,
            total_estimated_tokens: Option<i64>,
            total_duration_ms: Option<i64>,
            errors: i64,
            distinct_tools: i64,
        }
        let rows = sqlx::query_as::<_, Row>(
            r#"
            SELECT
                conversation_id AS task_id,
                COUNT(*)::bigint AS tool_calls,
                SUM(estimated_total_tokens)::bigint AS total_estimated_tokens,
                SUM(duration_ms)::bigint AS total_duration_ms,
                COUNT(*) FILTER (WHERE NOT ok)::bigint AS errors,
                COUNT(DISTINCT tool_name)::bigint AS distinct_tools
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
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| TopTaskRow {
            task_id: r.task_id, tool_calls: r.tool_calls,
            total_estimated_tokens: r.total_estimated_tokens,
            total_duration_ms: r.total_duration_ms,
            errors: r.errors, distinct_tools: r.distinct_tools,
        }).collect())
    }

    async fn calls_over_time(&self, q: &ReportQuery, bucket: &str) -> DbResult<Vec<CallsBucketRow>> {
        #[derive(sqlx::FromRow)]
        struct Row { bucket: chrono::DateTime<chrono::Utc>, calls: i64, errors: i64 }

        let interval = match bucket {
            "hour" => "hour",
            "day" => "day",
            "minute" => "minute",
            _ => "hour",
        };
        let rows = sqlx::query_as::<_, Row>(
            &format!(
                r#"
                SELECT
                    date_trunc('{interval}', started_at) AS bucket,
                    COUNT(*)::bigint AS calls,
                    COUNT(*) FILTER (WHERE NOT ok)::bigint AS errors
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
            ),
        )
        .bind(q.from)
        .bind(q.to)
        .bind(&q.repo)
        .bind(&q.ide)
        .bind(&q.agent)
        .bind(&q.model)
        .bind(&q.skill)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| CallsBucketRow {
            bucket: r.bucket, calls: r.calls, errors: r.errors,
        }).collect())
    }

    async fn distinct_models(&self) -> DbResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT model FROM agent_tool_calls WHERE model IS NOT NULL ORDER BY model",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn leaderboard_agents(&self, from: &str, limit: i64) -> DbResult<Vec<LeaderboardEntry>> {
        #[derive(sqlx::FromRow)]
        struct Row { name: String, events: i64, usd_cost: f64 }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT COALESCE(agent, 'unknown') AS name, \
             COUNT(*)::int8 AS events, \
             COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost \
             FROM agent_tool_calls WHERE created_at >= $1::timestamptz \
             GROUP BY agent ORDER BY events DESC LIMIT $2",
        )
        .bind(from)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| LeaderboardEntry { name: r.name, events: r.events, usd_cost: r.usd_cost }).collect())
    }

    async fn leaderboard_ides(&self, from: &str, limit: i64) -> DbResult<Vec<LeaderboardEntry>> {
        #[derive(sqlx::FromRow)]
        struct Row { name: String, events: i64, usd_cost: f64 }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT COALESCE(ide, 'unknown') AS name, \
             COUNT(*)::int8 AS events, \
             COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost \
             FROM agent_tool_calls WHERE created_at >= $1::timestamptz \
             GROUP BY ide ORDER BY events DESC LIMIT $2",
        )
        .bind(from)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| LeaderboardEntry { name: r.name, events: r.events, usd_cost: r.usd_cost }).collect())
    }

    async fn leaderboard_models(&self, from: &str, limit: i64) -> DbResult<Vec<LeaderboardEntry>> {
        #[derive(sqlx::FromRow)]
        struct Row { name: String, events: i64, usd_cost: f64 }
        let rows = sqlx::query_as::<_, Row>(
            "SELECT COALESCE(model, 'unknown') AS name, \
             COUNT(*)::int8 AS events, \
             COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost \
             FROM agent_tool_calls WHERE created_at >= $1::timestamptz \
             GROUP BY model ORDER BY events DESC LIMIT $2",
        )
        .bind(from)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| LeaderboardEntry { name: r.name, events: r.events, usd_cost: r.usd_cost }).collect())
    }

    async fn list_conversations(&self, q: &ConversationQuery) -> DbResult<Vec<ConversationRow>> {
        let rows = sqlx::query_as::<_, PgConversationRow>(
            r#"
            SELECT
                conversation_id,
                (SELECT up.user_prompt FROM agent_tool_calls up
                 WHERE up.conversation_id = atc.conversation_id
                   AND up.user_prompt IS NOT NULL AND LENGTH(up.user_prompt) BETWEEN 3 AND 2000
                 ORDER BY up.started_at ASC LIMIT 1) AS title,
                LEFT((SELECT up.user_prompt FROM agent_tool_calls up
                 WHERE up.conversation_id = atc.conversation_id
                   AND up.user_prompt IS NOT NULL AND LENGTH(up.user_prompt) BETWEEN 3 AND 2000
                 ORDER BY up.started_at ASC LIMIT 1), 300) AS initial_prompt,
                LEFT(MAX(tool_result) FILTER (
                    WHERE tool_result IS NOT NULL AND LENGTH(tool_result) >= 10 AND mcp_server IS NULL
                ), 300) AS response_preview,
                MIN(started_at) AS started_at,
                MAX(ended_at) AS ended_at,
                COALESCE(SUM(duration_ms), 0)::bigint AS total_duration_ms,
                COUNT(*)::bigint AS event_count,
                COUNT(*) FILTER (WHERE NOT ok)::bigint AS error_count,
                COALESCE(SUM(usd_cost), 0)::float8 AS total_usd_cost,
                MODE() WITHIN GROUP (ORDER BY agent) AS agent,
                MODE() WITHIN GROUP (ORDER BY ide) AS ide,
                MODE() WITHIN GROUP (ORDER BY model) AS model,
                SUM(estimated_input_tokens)::bigint AS total_tokens_in,
                SUM(estimated_output_tokens)::bigint AS total_tokens_out
            FROM agent_tool_calls atc
            WHERE conversation_id IS NOT NULL AND conversation_id <> ''
              AND ($3::text IS NULL OR ide = $3)
            GROUP BY conversation_id
            HAVING NOT (
                COUNT(*) = 1
                AND (
                    MAX(agent) IN ('copilotLanguageModelWrapper', 'title', 'summarizeConversationHistory')
                    OR BOOL_OR(COALESCE(user_prompt,'') ILIKE 'Please write a brief title%')
                    OR BOOL_OR(COALESCE(user_prompt,'') ILIKE 'Summarize the following content%')
                    OR (MAX(model) IS NULL AND MAX(user_prompt) IS NULL)
                )
            )
            ORDER BY started_at DESC
            LIMIT $1 OFFSET $2
            "#,
        )
        .bind(q.limit)
        .bind(q.offset)
        .bind(&q.ide)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| ConversationRow {
            conversation_id: r.conversation_id, title: r.title, initial_prompt: r.initial_prompt,
            response_preview: r.response_preview, started_at: r.started_at, ended_at: r.ended_at,
            total_duration_ms: r.total_duration_ms, event_count: r.event_count,
            error_count: r.error_count, total_usd_cost: r.total_usd_cost,
            agent: r.agent, ide: r.ide, model: r.model,
            total_tokens_in: r.total_tokens_in, total_tokens_out: r.total_tokens_out,
        }).collect())
    }

    async fn conversation_detail(&self, conversation_id: &str) -> DbResult<Vec<ToolCallRow>> {
        let rows = sqlx::query_as::<_, PgToolCallRow>(
            r#"SELECT * FROM agent_tool_calls WHERE conversation_id = $1 ORDER BY started_at ASC"#,
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn cost_summary(&self, q: &CostQuery) -> DbResult<CostSummaryResult> {
        // KPIs
        let kpi_row = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(usd_cost), 0)::float8 AS total_usd,
                0::float8 AS total_credits,
                COUNT(*)::bigint AS total_events,
                COALESCE(SUM(estimated_input_tokens), 0)::bigint AS total_tokens_in,
                COALESCE(SUM(estimated_output_tokens), 0)::bigint AS total_tokens_out,
                COALESCE(AVG(usd_cost), 0)::float8 AS avg_usd_per_event,
                COALESCE(SUM(usd_cost) / NULLIF(EXTRACT(EPOCH FROM ($2::timestamptz - $1::timestamptz)) / 3600.0, 0), 0)::float8 AS burn_rate_usd_per_hour,
                COALESCE(AVG(duration_ms), 0)::float8 AS avg_duration_ms,
                COALESCE(COUNT(*) FILTER (WHERE NOT ok)::float8 / NULLIF(COUNT(*), 0)::float8, 0)::float8 AS error_rate
            FROM agent_tool_calls
            WHERE started_at >= $1 AND started_at <= $2
              AND ($3::text IS NULL OR model = $3)
            "#,
        )
        .bind(q.from)
        .bind(q.to)
        .bind(&q.model)
        .fetch_one(&self.pool)
        .await?;

        let kpis = CostKpisRow {
            total_usd: kpi_row.get("total_usd"),
            total_credits: kpi_row.get("total_credits"),
            total_events: kpi_row.get("total_events"),
            total_tokens_in: kpi_row.get("total_tokens_in"),
            total_tokens_out: kpi_row.get("total_tokens_out"),
            avg_usd_per_event: kpi_row.get("avg_usd_per_event"),
            burn_rate_usd_per_hour: kpi_row.get("burn_rate_usd_per_hour"),
            avg_duration_ms: kpi_row.get("avg_duration_ms"),
            error_rate: kpi_row.get("error_rate"),
        };

        // By model
        let by_model: Vec<ModelCostRow> = sqlx::query(
            r#"
            SELECT model, COUNT(*)::bigint AS events,
                   COALESCE(SUM(estimated_input_tokens), 0)::bigint AS tokens_in,
                   COALESCE(SUM(estimated_output_tokens), 0)::bigint AS tokens_out,
                   COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost
            FROM agent_tool_calls
            WHERE started_at >= $1 AND started_at <= $2
              AND ($3::text IS NULL OR model = $3)
            GROUP BY model ORDER BY usd_cost DESC LIMIT 20
            "#,
        )
        .bind(q.from).bind(q.to).bind(&q.model)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| ModelCostRow {
            model: r.get("model"), events: r.get("events"),
            tokens_in: r.get("tokens_in"), tokens_out: r.get("tokens_out"),
            usd_cost: r.get("usd_cost"),
        })
        .collect();

        // By day
        let by_day: Vec<CostByDayRow> = sqlx::query(
            r#"
            SELECT date_trunc('day', started_at) AS day,
                   COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost,
                   COUNT(*)::bigint AS events
            FROM agent_tool_calls
            WHERE started_at >= $1 AND started_at <= $2
              AND ($3::text IS NULL OR model = $3)
            GROUP BY day ORDER BY day
            "#,
        )
        .bind(q.from).bind(q.to).bind(&q.model)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| CostByDayRow { day: r.get("day"), usd_cost: r.get("usd_cost"), events: r.get("events") })
        .collect();

        // By billing model
        let by_billing_model: Vec<BillingModelBreakdownRow> = sqlx::query(
            r#"
            SELECT billing_model,
                   COUNT(*)::bigint AS events,
                   COALESCE(SUM(usd_cost), 0)::float8 AS usd_cost,
                   0::float8 AS credits
            FROM agent_tool_calls
            WHERE started_at >= $1 AND started_at <= $2
              AND ($3::text IS NULL OR model = $3)
            GROUP BY billing_model ORDER BY usd_cost DESC
            "#,
        )
        .bind(q.from).bind(q.to).bind(&q.model)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| BillingModelBreakdownRow {
            billing_model: r.get("billing_model"), events: r.get("events"),
            usd_cost: r.get("usd_cost"), credits: r.get("credits"),
        })
        .collect();

        Ok(CostSummaryResult { kpis, by_model, by_day, by_billing_model })
    }

    async fn list_orgs(&self) -> DbResult<Vec<OrgRow>> {
        let rows = sqlx::query_as::<_, PgOrgRow>(
            "SELECT id, slug, name, plan, created_at FROM organizations ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| OrgRow {
            id: r.id, slug: r.slug, name: r.name, plan: r.plan, created_at: r.created_at,
        }).collect())
    }

    async fn find_org_by_slug(&self, slug: &str) -> DbResult<OrgRow> {
        let r = sqlx::query_as::<_, PgOrgRow>(
            "SELECT id, slug, name, plan, created_at FROM organizations WHERE slug = $1",
        )
        .bind(slug)
        .fetch_one(&self.pool)
        .await?;

        Ok(OrgRow { id: r.id, slug: r.slug, name: r.name, plan: r.plan, created_at: r.created_at })
    }

    async fn list_api_keys(&self, org_id: uuid::Uuid) -> DbResult<Vec<ApiKeyRow>> {
        let rows = sqlx::query_as::<_, PgApiKeyRow>(
            "SELECT id, org_id, key_prefix, name, created_at, last_used_at FROM api_keys WHERE org_id = $1 ORDER BY created_at DESC",
        )
        .bind(org_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| ApiKeyRow {
            id: r.id, org_id: r.org_id, key_prefix: r.key_prefix,
            name: r.name, created_at: r.created_at, last_used_at: r.last_used_at,
        }).collect())
    }

    async fn create_api_key(&self, org_id: uuid::Uuid, name: &str, prefix: &str, hash: &str) -> DbResult<ApiKeyRow> {
        let r = sqlx::query_as::<_, PgApiKeyRow>(
            r#"
            INSERT INTO api_keys (org_id, name, key_prefix, key_hash)
            VALUES ($1, $2, $3, $4)
            RETURNING id, org_id, key_prefix, name, created_at, last_used_at
            "#,
        )
        .bind(org_id)
        .bind(name)
        .bind(prefix)
        .bind(hash)
        .fetch_one(&self.pool)
        .await?;

        Ok(ApiKeyRow {
            id: r.id, org_id: r.org_id, key_prefix: r.key_prefix,
            name: r.name, created_at: r.created_at, last_used_at: r.last_used_at,
        })
    }

    async fn revoke_api_key(&self, key_id: uuid::Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM api_keys WHERE id = $1")
            .bind(key_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn find_key_by_prefix(&self, prefix: &str) -> DbResult<Option<ApiKeyMetaRow>> {
        let row = sqlx::query_as::<_, PgApiKeyMetaRow>(
            "SELECT id, org_id, key_hash FROM api_keys WHERE key_prefix = $1",
        )
        .bind(prefix)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| ApiKeyMetaRow { id: r.id, org_id: r.org_id, key_hash: r.key_hash }))
    }

    async fn search(&self, query: &str, limit: i64) -> DbResult<Vec<SearchResultRow>> {
        let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));

        let rows = sqlx::query_as::<_, PgSearchResultRow>(
            r#"
            SELECT DISTINCT ON (conversation_id)
                conversation_id, user_prompt, model, agent, tool_name, started_at,
                CASE
                    WHEN user_prompt ILIKE $1 THEN 'prompt'
                    WHEN tool_name ILIKE $1 THEN 'tool'
                    WHEN model ILIKE $1 THEN 'model'
                    WHEN agent ILIKE $1 THEN 'agent'
                    WHEN conversation_id ILIKE $1 THEN 'conversation'
                    WHEN skill ILIKE $1 THEN 'skill'
                    WHEN mcp_server ILIKE $1 THEN 'mcp_server'
                    ELSE 'other'
                END AS match_field
            FROM agent_tool_calls
            WHERE user_prompt ILIKE $1 OR tool_name ILIKE $1 OR model ILIKE $1
               OR agent ILIKE $1 OR conversation_id ILIKE $1 OR skill ILIKE $1
               OR mcp_server ILIKE $1
            ORDER BY conversation_id, started_at DESC
            LIMIT $2
            "#,
        )
        .bind(&pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| SearchResultRow {
            conversation_id: r.conversation_id, user_prompt: r.user_prompt,
            model: r.model, agent: r.agent, tool_name: r.tool_name,
            started_at: r.started_at, match_field: r.match_field,
        }).collect())
    }

    async fn migrate(&self) -> DbResult<()> {
        sqlx::migrate!("../../migrations")
            .run(&self.pool)
            .await
            .map_err(|e| DbError::Internal(format!("migration error: {e}")))?;
        Ok(())
    }

    async fn health_check(&self) -> DbResult<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
