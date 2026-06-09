//! SQLite implementation of the Database trait.
//!
//! Suitable for single-user, self-hosted, or development deployments.
//! Uses WAL mode for concurrency and FTS5 for search.

use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;

use crate::models::*;
use crate::params::*;
use crate::{Database, DbError, DbResult};

/// SQLite-backed database.
#[derive(Clone)]
pub struct SqliteDb {
    pool: SqlitePool,
}

impl SqliteDb {
    /// Connect to SQLite (creates file if it doesn't exist).
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        let opts = SqliteConnectOptions::from_str(url)
            .map_err(|e| DbError::Internal(format!("sqlite url parse: {e}")))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .busy_timeout(std::time::Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await
            .map_err(|e| DbError::Internal(format!("sqlite connect: {e}")))?;

        // Enable WAL and set pragmas
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await
            .map_err(|e| DbError::Internal(format!("sqlite pragma: {e}")))?;
        sqlx::query("PRAGMA busy_timeout=5000")
            .execute(&pool)
            .await
            .map_err(|e| DbError::Internal(format!("sqlite pragma: {e}")))?;
        sqlx::query("PRAGMA foreign_keys=ON")
            .execute(&pool)
            .await
            .map_err(|e| DbError::Internal(format!("sqlite pragma: {e}")))?;

        tracing::info!("connected to SQLite");
        Ok(Self { pool })
    }

    /// Run embedded migrations (creates tables).
    async fn run_migrations(&self) -> DbResult<()> {
        // Schema creation inline — no external migration files for SQLite
        sqlx::query(SQLITE_SCHEMA)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::Internal(format!("sqlite schema: {e}")))?;
        Ok(())
    }
}

/// SQLite schema — equivalent to the Postgres schema but with SQLite types.
const SQLITE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS agent_tool_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL,
    task_id TEXT,
    repo TEXT,
    branch TEXT,
    ide TEXT,
    agent TEXT,
    skill TEXT,
    mcp_server TEXT,
    tool_name TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    ok INTEGER NOT NULL DEFAULT 1,
    error TEXT,
    request_bytes INTEGER,
    response_bytes INTEGER,
    estimated_input_tokens INTEGER,
    estimated_output_tokens INTEGER,
    estimated_total_tokens INTEGER,
    request_sha256 TEXT,
    response_sha256 TEXT,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    model TEXT,
    cached_tokens INTEGER,
    conversation_id TEXT,
    client_ip TEXT,
    user_agent TEXT,
    user_prompt TEXT,
    tool_arguments TEXT,
    tool_result TEXT,
    reasoning_tokens INTEGER,
    finish_reason TEXT,
    request_max_tokens INTEGER,
    request_temperature REAL,
    llm_system TEXT,
    trace_id TEXT,
    span_id TEXT,
    parent_span_id TEXT,
    tool_call_id TEXT,
    usd_cost REAL,
    billing_model TEXT NOT NULL DEFAULT 'token'
);

CREATE INDEX IF NOT EXISTS idx_atc_started_at ON agent_tool_calls(started_at);
CREATE INDEX IF NOT EXISTS idx_atc_conversation_id ON agent_tool_calls(conversation_id);
CREATE INDEX IF NOT EXISTS idx_atc_ide ON agent_tool_calls(ide);
CREATE INDEX IF NOT EXISTS idx_atc_agent ON agent_tool_calls(agent);
CREATE INDEX IF NOT EXISTS idx_atc_model ON agent_tool_calls(model);
CREATE INDEX IF NOT EXISTS idx_atc_event_id ON agent_tool_calls(event_id);

CREATE TABLE IF NOT EXISTS organizations (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    plan TEXT NOT NULL DEFAULT 'free',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    org_id TEXT NOT NULL REFERENCES organizations(id),
    key_prefix TEXT NOT NULL,
    key_hash TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_used_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_api_keys_prefix ON api_keys(key_prefix);

-- FTS5 virtual table for full-text search
CREATE VIRTUAL TABLE IF NOT EXISTS atc_fts USING fts5(
    conversation_id,
    user_prompt,
    tool_name,
    model,
    agent,
    skill,
    mcp_server,
    content='agent_tool_calls',
    content_rowid='id'
);

-- Trigger to keep FTS in sync
CREATE TRIGGER IF NOT EXISTS atc_fts_insert AFTER INSERT ON agent_tool_calls BEGIN
    INSERT INTO atc_fts(rowid, conversation_id, user_prompt, tool_name, model, agent, skill, mcp_server)
    VALUES (NEW.id, NEW.conversation_id, NEW.user_prompt, NEW.tool_name, NEW.model, NEW.agent, NEW.skill, NEW.mcp_server);
END;
"#;

// ── Helper: parse ISO8601 from SQLite TEXT ──────────────────────────────────

fn parse_dt(s: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ")
                .map(|n| n.and_utc())
                .unwrap_or_default()
        })
}

fn parse_dt_opt(s: Option<&str>) -> Option<chrono::DateTime<chrono::Utc>> {
    s.map(|s| parse_dt(s))
}

// ── Trait implementation ────────────────────────────────────────────────────

#[async_trait]
impl Database for SqliteDb {
    async fn insert_tool_call(&self, e: &InsertToolCall) -> DbResult<ToolCallRow> {
        let billing_model = match e.ide.as_deref() {
            Some(ide) if ide.to_lowercase().contains("copilot") || ide.to_lowercase().contains("vscode") => "copilot_credit",
            Some(ide) if ide.to_lowercase().contains("cursor") => "cursor_usage",
            _ => "token",
        };

        let row = sqlx::query(
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
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22,
                ?23, ?24, ?25, ?26, ?27, ?28,
                ?29, ?30,
                ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39,
                ?40, ?41
            )
            RETURNING *
            "#,
        )
        .bind(e.event_id.to_string())
        .bind(&e.task_id)
        .bind(&e.repo)
        .bind(&e.branch)
        .bind(&e.ide)
        .bind(&e.agent)
        .bind(&e.skill)
        .bind(&e.mcp_server)
        .bind(&e.tool_name)
        .bind(e.started_at.to_rfc3339())
        .bind(e.ended_at.to_rfc3339())
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
        .bind(e.metadata.to_string())
        .bind(&e.model)
        .bind(e.cached_tokens)
        .bind(&e.conversation_id)
        .bind(&e.client_ip)
        .bind(&e.user_agent)
        .bind(&e.user_prompt)
        .bind(e.tool_arguments.as_ref().map(|v| v.to_string()))
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
        .bind(0.0f64) // usd_cost placeholder — no compute_event_usd in SQLite
        .bind(billing_model)
        .fetch_one(&self.pool)
        .await?;

        Ok(sqlite_row_to_tool_call(&row))
    }

    async fn query_events(&self, q: &EventQuery) -> DbResult<Vec<EventFeedRow>> {
        let rows = sqlx::query(
            r#"
            SELECT event_id, tool_name, model, started_at, duration_ms, ok,
                   estimated_input_tokens, estimated_output_tokens, cached_tokens,
                   agent, ide, mcp_server, conversation_id, client_ip, user_prompt,
                   tool_arguments, tool_result
            FROM agent_tool_calls
            WHERE (?1 IS NULL OR started_at >= ?1)
              AND (?2 IS NULL OR started_at <= ?2)
              AND (?3 IS NULL OR ide = ?3)
              AND (?4 IS NULL OR agent = ?4)
              AND (?5 IS NULL OR model = ?5)
              AND (?6 IS NULL OR conversation_id = ?6)
            ORDER BY started_at DESC, id DESC
            LIMIT ?7 OFFSET ?8
            "#,
        )
        .bind(q.from.map(|d| d.to_rfc3339()))
        .bind(q.to.map(|d| d.to_rfc3339()))
        .bind(&q.ide)
        .bind(&q.agent)
        .bind(&q.model)
        .bind(&q.conversation_id)
        .bind(q.limit)
        .bind(q.offset)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| EventFeedRow {
            event_id: uuid::Uuid::parse_str(r.get::<&str, _>("event_id")).unwrap_or_default(),
            tool_name: r.get("tool_name"),
            model: r.get("model"),
            started_at: parse_dt(r.get("started_at")),
            duration_ms: r.get("duration_ms"),
            ok: r.get::<bool, _>("ok"),
            estimated_input_tokens: r.get("estimated_input_tokens"),
            estimated_output_tokens: r.get("estimated_output_tokens"),
            cached_tokens: r.get("cached_tokens"),
            agent: r.get("agent"),
            ide: r.get("ide"),
            mcp_server: r.get("mcp_server"),
            conversation_id: r.get("conversation_id"),
            client_ip: r.get("client_ip"),
            user_prompt: r.get("user_prompt"),
            tool_arguments: r.get::<Option<String>, _>("tool_arguments").and_then(|s| serde_json::from_str(&s).ok()),
            tool_result: r.get("tool_result"),
        }).collect())
    }

    async fn top_tools(&self, q: &ReportQuery) -> DbResult<Vec<TopToolRow>> {
        let rows = sqlx::query(
            r#"
            SELECT
                mcp_server, tool_name,
                COUNT(*) AS calls,
                SUM(estimated_total_tokens) AS total_estimated_tokens,
                AVG(duration_ms) AS avg_duration_ms,
                SUM(CASE WHEN NOT ok THEN 1 ELSE 0 END) AS errors,
                AVG(response_bytes) AS avg_response_bytes,
                (
                    SELECT atc2.model
                    FROM agent_tool_calls atc2
                    WHERE atc2.mcp_server = agent_tool_calls.mcp_server
                      AND atc2.tool_name = agent_tool_calls.tool_name
                      AND (?1 IS NULL OR atc2.started_at >= ?1)
                      AND (?2 IS NULL OR atc2.started_at <= ?2)
                      AND (?3 IS NULL OR atc2.repo = ?3)
                      AND (?4 IS NULL OR atc2.ide = ?4)
                      AND (?5 IS NULL OR atc2.agent = ?5)
                      AND (?6 IS NULL OR atc2.model = ?6)
                      AND (?7 IS NULL OR atc2.skill = ?7)
                      AND atc2.model IS NOT NULL
                    GROUP BY atc2.model
                    ORDER BY COUNT(*) DESC, atc2.model ASC
                    LIMIT 1
                ) AS top_model,
                SUM(cached_tokens) AS cached_tokens_total,
                AVG(estimated_input_tokens) AS avg_input_tokens,
                AVG(estimated_output_tokens) AS avg_output_tokens
            FROM agent_tool_calls
            WHERE (?1 IS NULL OR started_at >= ?1)
              AND (?2 IS NULL OR started_at <= ?2)
              AND (?3 IS NULL OR repo = ?3)
              AND (?4 IS NULL OR ide = ?4)
              AND (?5 IS NULL OR agent = ?5)
              AND (?6 IS NULL OR model = ?6)
              AND (?7 IS NULL OR skill = ?7)
            GROUP BY mcp_server, tool_name
            ORDER BY calls DESC
            LIMIT ?8
            "#,
        )
        .bind(q.from.map(|d| d.to_rfc3339()))
        .bind(q.to.map(|d| d.to_rfc3339()))
        .bind(&q.repo)
        .bind(&q.ide)
        .bind(&q.agent)
        .bind(&q.model)
        .bind(&q.skill)
        .bind(q.limit.unwrap_or(20))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| TopToolRow {
            mcp_server: r.get("mcp_server"),
            tool_name: r.get("tool_name"),
            calls: r.get("calls"),
            total_estimated_tokens: r.get("total_estimated_tokens"),
            avg_duration_ms: r.get("avg_duration_ms"),
            errors: r.get("errors"),
            avg_response_bytes: r.get("avg_response_bytes"),
            top_model: r.get("top_model"),
            cached_tokens_total: r.get("cached_tokens_total"),
            avg_input_tokens: r.get("avg_input_tokens"),
            avg_output_tokens: r.get("avg_output_tokens"),
        }).collect())
    }

    async fn top_agents(&self, q: &ReportQuery) -> DbResult<Vec<TopAgentRow>> {
        let rows = sqlx::query(
            r#"
            SELECT
                agent,
                COUNT(*) AS calls,
                SUM(estimated_total_tokens) AS total_tokens,
                SUM(usd_cost) AS total_usd_cost,
                SUM(CASE WHEN NOT ok THEN 1 ELSE 0 END) AS errors,
                COUNT(DISTINCT conversation_id) AS conversations
            FROM agent_tool_calls
            WHERE agent IS NOT NULL
              AND (?1 IS NULL OR started_at >= ?1)
              AND (?2 IS NULL OR started_at <= ?2)
              AND (?3 IS NULL OR repo = ?3)
              AND (?4 IS NULL OR ide = ?4)
              AND (?5 IS NULL OR model = ?5)
            GROUP BY agent
            ORDER BY calls DESC
            LIMIT ?6
            "#,
        )
        .bind(q.from.map(|d| d.to_rfc3339()))
        .bind(q.to.map(|d| d.to_rfc3339()))
        .bind(&q.repo)
        .bind(&q.ide)
        .bind(&q.model)
        .bind(q.limit.unwrap_or(20))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| TopAgentRow {
            agent: r.get("agent"),
            calls: r.get("calls"),
            total_tokens: r.get("total_tokens"),
            total_usd_cost: r.get("total_usd_cost"),
            errors: r.get("errors"),
            conversations: r.get("conversations"),
        }).collect())
    }

    async fn top_mcp_servers(&self, q: &ReportQuery) -> DbResult<Vec<TopMcpServerRow>> {
        let rows = sqlx::query(
            r#"
            SELECT
                mcp_server,
                COUNT(*) AS calls,
                SUM(estimated_total_tokens) AS total_estimated_tokens,
                AVG(response_bytes) AS avg_response_bytes,
                CAST(SUM(CASE WHEN NOT ok THEN 1 ELSE 0 END) AS REAL) / NULLIF(COUNT(*), 0) AS error_rate
            FROM agent_tool_calls
            WHERE mcp_server IS NOT NULL AND mcp_server <> ''
              AND tool_name <> 'llm_chat'
              AND (?1 IS NULL OR started_at >= ?1)
              AND (?2 IS NULL OR started_at <= ?2)
              AND (?3 IS NULL OR repo = ?3)
              AND (?4 IS NULL OR ide = ?4)
            GROUP BY mcp_server
            ORDER BY calls DESC
            LIMIT ?5
            "#,
        )
        .bind(q.from.map(|d| d.to_rfc3339()))
        .bind(q.to.map(|d| d.to_rfc3339()))
        .bind(&q.repo)
        .bind(&q.ide)
        .bind(q.limit.unwrap_or(20))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| TopMcpServerRow {
            mcp_server: r.get("mcp_server"),
            calls: r.get("calls"),
            total_estimated_tokens: r.get("total_estimated_tokens"),
            avg_response_bytes: r.get("avg_response_bytes"),
            error_rate: r.get("error_rate"),
        }).collect())
    }

    async fn ide_breakdown(&self, q: &ReportQuery) -> DbResult<Vec<IdeBreakdownRow>> {
        let rows = sqlx::query(
            r#"
            SELECT
                COALESCE(ide, 'unknown') AS ide,
                COUNT(*) AS calls,
                SUM(estimated_total_tokens) AS total_estimated_tokens,
                SUM(CASE WHEN NOT ok THEN 1 ELSE 0 END) AS errors,
                SUM(CASE WHEN mcp_server IS NULL AND tool_name = 'llm_chat' THEN 1 ELSE 0 END) AS llm_calls,
                SUM(CASE WHEN mcp_server IS NOT NULL OR tool_name <> 'llm_chat' THEN 1 ELSE 0 END) AS tool_calls_count
            FROM agent_tool_calls
            WHERE (?1 IS NULL OR started_at >= ?1)
              AND (?2 IS NULL OR started_at <= ?2)
              AND (?3 IS NULL OR repo = ?3)
            GROUP BY ide
            ORDER BY calls DESC
            "#,
        )
        .bind(q.from.map(|d| d.to_rfc3339()))
        .bind(q.to.map(|d| d.to_rfc3339()))
        .bind(&q.repo)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| IdeBreakdownRow {
            ide: r.get("ide"),
            calls: r.get("calls"),
            total_estimated_tokens: r.get("total_estimated_tokens"),
            errors: r.get("errors"),
            llm_calls: r.get("llm_calls"),
            tool_calls_count: r.get("tool_calls_count"),
        }).collect())
    }

    async fn error_patterns(&self, q: &ReportQuery) -> DbResult<Vec<ErrorPatternRow>> {
        let rows = sqlx::query(
            r#"
            WITH filtered AS (
                SELECT error, tool_name, model, started_at
                FROM agent_tool_calls
                WHERE NOT ok AND error IS NOT NULL
                  AND (?1 IS NULL OR started_at >= ?1)
                  AND (?2 IS NULL OR started_at <= ?2)
                  AND (?3 IS NULL OR repo = ?3)
                  AND (?4 IS NULL OR ide = ?4)
            ),
            base AS (
                SELECT
                    error,
                    COUNT(*) AS occurrences,
                    MAX(started_at) AS last_seen
                FROM filtered
                GROUP BY error
            ),
            tool_ranked AS (
                SELECT
                    error,
                    tool_name,
                    COUNT(*) AS cnt,
                    ROW_NUMBER() OVER (
                        PARTITION BY error
                        ORDER BY COUNT(*) DESC, tool_name ASC
                    ) AS rn
                FROM filtered
                GROUP BY error, tool_name
            ),
            model_ranked AS (
                SELECT
                    error,
                    model,
                    COUNT(*) AS cnt,
                    ROW_NUMBER() OVER (
                        PARTITION BY error
                        ORDER BY COUNT(*) DESC, model ASC
                    ) AS rn
                FROM filtered
                GROUP BY error, model
            )
            SELECT
                b.error,
                b.occurrences,
                tr.tool_name,
                mr.model,
                b.last_seen
            FROM base b
            LEFT JOIN tool_ranked tr ON tr.error = b.error AND tr.rn = 1
            LEFT JOIN model_ranked mr ON mr.error = b.error AND mr.rn = 1
            ORDER BY b.occurrences DESC
            LIMIT ?5
            "#,
        )
        .bind(q.from.map(|d| d.to_rfc3339()))
        .bind(q.to.map(|d| d.to_rfc3339()))
        .bind(&q.repo)
        .bind(&q.ide)
        .bind(q.limit.unwrap_or(20))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| ErrorPatternRow {
            error: r.get("error"),
            occurrences: r.get("occurrences"),
            tool_name: r.get("tool_name"),
            model: r.get("model"),
            last_seen: parse_dt(r.get("last_seen")),
        }).collect())
    }

    async fn cost_over_time(&self, q: &ReportQuery) -> DbResult<Vec<CostBucketRow>> {
        let rows = sqlx::query(
            r#"
            SELECT
                strftime('%Y-%m-%dT%H:00:00Z', started_at) AS bucket,
                SUM(usd_cost) AS total_usd,
                COUNT(*) AS calls
            FROM agent_tool_calls
            WHERE (?1 IS NULL OR started_at >= ?1)
              AND (?2 IS NULL OR started_at <= ?2)
              AND (?3 IS NULL OR repo = ?3)
              AND (?4 IS NULL OR ide = ?4)
            GROUP BY bucket
            ORDER BY bucket
            "#,
        )
        .bind(q.from.map(|d| d.to_rfc3339()))
        .bind(q.to.map(|d| d.to_rfc3339()))
        .bind(&q.repo)
        .bind(&q.ide)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| CostBucketRow {
            bucket: parse_dt(r.get("bucket")),
            total_usd: r.get("total_usd"),
            calls: r.get("calls"),
        }).collect())
    }

    async fn top_tasks(&self, _q: &ReportQuery) -> DbResult<Vec<TopTaskRow>> {
        // TODO: Implement SQLite version
        Ok(vec![])
    }

    async fn calls_over_time(&self, _q: &ReportQuery, _bucket: &str) -> DbResult<Vec<CallsBucketRow>> {
        // TODO: Implement SQLite version
        Ok(vec![])
    }

    async fn distinct_models(&self) -> DbResult<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT model FROM agent_tool_calls WHERE model IS NOT NULL ORDER BY model",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    async fn leaderboard_agents(&self, _from: &str, _limit: i64) -> DbResult<Vec<LeaderboardEntry>> {
        Ok(vec![])
    }

    async fn leaderboard_ides(&self, _from: &str, _limit: i64) -> DbResult<Vec<LeaderboardEntry>> {
        Ok(vec![])
    }

    async fn leaderboard_models(&self, _from: &str, _limit: i64) -> DbResult<Vec<LeaderboardEntry>> {
        Ok(vec![])
    }

    async fn list_conversations(&self, q: &ConversationQuery) -> DbResult<Vec<ConversationRow>> {
        let rows = sqlx::query(
            r#"
            SELECT
                conversation_id,
                MIN(user_prompt) AS title,
                MIN(user_prompt) AS initial_prompt,
                NULL AS response_preview,
                MIN(started_at) AS started_at,
                MAX(ended_at) AS ended_at,
                COALESCE(SUM(duration_ms), 0) AS total_duration_ms,
                COUNT(*) AS event_count,
                SUM(CASE WHEN NOT ok THEN 1 ELSE 0 END) AS error_count,
                COALESCE(SUM(usd_cost), 0.0) AS total_usd_cost,
                agent,
                ide,
                model,
                SUM(estimated_input_tokens) AS total_tokens_in,
                SUM(estimated_output_tokens) AS total_tokens_out
            FROM agent_tool_calls
            WHERE conversation_id IS NOT NULL AND conversation_id <> ''
              AND (?3 IS NULL OR ide = ?3)
            GROUP BY conversation_id
            HAVING COUNT(*) > 1
            ORDER BY started_at DESC
            LIMIT ?1 OFFSET ?2
            "#,
        )
        .bind(q.limit)
        .bind(q.offset)
        .bind(&q.ide)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| ConversationRow {
            conversation_id: r.get("conversation_id"),
            title: r.get("title"),
            initial_prompt: r.get("initial_prompt"),
            response_preview: r.get("response_preview"),
            started_at: parse_dt(r.get("started_at")),
            ended_at: parse_dt_opt(r.try_get::<&str, _>("ended_at").ok()),
            total_duration_ms: r.get("total_duration_ms"),
            event_count: r.get("event_count"),
            error_count: r.get("error_count"),
            total_usd_cost: r.get("total_usd_cost"),
            agent: r.get("agent"),
            ide: r.get("ide"),
            model: r.get("model"),
            total_tokens_in: r.get("total_tokens_in"),
            total_tokens_out: r.get("total_tokens_out"),
        }).collect())
    }

    async fn conversation_detail(&self, conversation_id: &str) -> DbResult<Vec<ToolCallRow>> {
        let rows = sqlx::query(
            "SELECT * FROM agent_tool_calls WHERE conversation_id = ?1 ORDER BY started_at ASC",
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| sqlite_row_to_tool_call(r)).collect())
    }

    async fn cost_summary(&self, q: &CostQuery) -> DbResult<CostSummaryResult> {
        let from_str = q.from.to_rfc3339();
        let to_str = q.to.to_rfc3339();

        let kpi_row = sqlx::query(
            r#"
            SELECT
                COALESCE(SUM(usd_cost), 0.0) AS total_usd,
                0.0 AS total_credits,
                COUNT(*) AS total_events,
                COALESCE(SUM(estimated_input_tokens), 0) AS total_tokens_in,
                COALESCE(SUM(estimated_output_tokens), 0) AS total_tokens_out,
                COALESCE(AVG(usd_cost), 0.0) AS avg_usd_per_event,
                0.0 AS burn_rate_usd_per_hour,
                COALESCE(AVG(duration_ms), 0.0) AS avg_duration_ms,
                CAST(SUM(CASE WHEN NOT ok THEN 1 ELSE 0 END) AS REAL) / NULLIF(COUNT(*), 0) AS error_rate
            FROM agent_tool_calls
            WHERE started_at >= ?1 AND started_at <= ?2
              AND (?3 IS NULL OR model = ?3)
            "#,
        )
        .bind(&from_str)
        .bind(&to_str)
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

        let by_model: Vec<ModelCostRow> = sqlx::query(
            r#"
            SELECT model, COUNT(*) AS events,
                   COALESCE(SUM(estimated_input_tokens), 0) AS tokens_in,
                   COALESCE(SUM(estimated_output_tokens), 0) AS tokens_out,
                   COALESCE(SUM(usd_cost), 0.0) AS usd_cost
            FROM agent_tool_calls
            WHERE started_at >= ?1 AND started_at <= ?2
              AND (?3 IS NULL OR model = ?3)
            GROUP BY model ORDER BY usd_cost DESC LIMIT 20
            "#,
        )
        .bind(&from_str).bind(&to_str).bind(&q.model)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| ModelCostRow {
            model: r.get("model"), events: r.get("events"),
            tokens_in: r.get("tokens_in"), tokens_out: r.get("tokens_out"),
            usd_cost: r.get("usd_cost"),
        })
        .collect();

        let by_day: Vec<CostByDayRow> = sqlx::query(
            r#"
            SELECT strftime('%Y-%m-%dT00:00:00Z', started_at) AS day,
                   COALESCE(SUM(usd_cost), 0.0) AS usd_cost,
                   COUNT(*) AS events
            FROM agent_tool_calls
            WHERE started_at >= ?1 AND started_at <= ?2
              AND (?3 IS NULL OR model = ?3)
            GROUP BY day ORDER BY day
            "#,
        )
        .bind(&from_str).bind(&to_str).bind(&q.model)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| CostByDayRow {
            day: parse_dt(r.get("day")),
            usd_cost: r.get("usd_cost"),
            events: r.get("events"),
        })
        .collect();

        let by_billing_model: Vec<BillingModelBreakdownRow> = sqlx::query(
            r#"
            SELECT billing_model, COUNT(*) AS events,
                   COALESCE(SUM(usd_cost), 0.0) AS usd_cost,
                   0.0 AS credits
            FROM agent_tool_calls
            WHERE started_at >= ?1 AND started_at <= ?2
              AND (?3 IS NULL OR model = ?3)
            GROUP BY billing_model ORDER BY usd_cost DESC
            "#,
        )
        .bind(&from_str).bind(&to_str).bind(&q.model)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| BillingModelBreakdownRow {
            billing_model: r.get("billing_model"), events: r.get("events"),
            usd_cost: r.get("usd_cost"), credits: r.get("credits"),
        })
        .collect();

        Ok(CostSummaryResult { kpis, by_model, by_day, by_billing_model })
    }

    async fn list_orgs(&self) -> DbResult<Vec<OrgRow>> {
        let rows = sqlx::query(
            "SELECT id, slug, name, plan, created_at FROM organizations ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| OrgRow {
            id: uuid::Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
            slug: r.get("slug"), name: r.get("name"), plan: r.get("plan"),
            created_at: parse_dt(r.get("created_at")),
        }).collect())
    }

    async fn find_org_by_slug(&self, slug: &str) -> DbResult<OrgRow> {
        let r = sqlx::query(
            "SELECT id, slug, name, plan, created_at FROM organizations WHERE slug = ?1",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;

        Ok(OrgRow {
            id: uuid::Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
            slug: r.get("slug"), name: r.get("name"), plan: r.get("plan"),
            created_at: parse_dt(r.get("created_at")),
        })
    }

    async fn list_api_keys(&self, org_id: uuid::Uuid) -> DbResult<Vec<ApiKeyRow>> {
        let rows = sqlx::query(
            "SELECT id, org_id, key_prefix, name, created_at, last_used_at FROM api_keys WHERE org_id = ?1 ORDER BY created_at DESC",
        )
        .bind(org_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| ApiKeyRow {
            id: uuid::Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
            org_id: uuid::Uuid::parse_str(r.get::<&str, _>("org_id")).unwrap_or_default(),
            key_prefix: r.get("key_prefix"), name: r.get("name"),
            created_at: parse_dt(r.get("created_at")),
            last_used_at: parse_dt_opt(r.try_get::<&str, _>("last_used_at").ok()),
        }).collect())
    }

    async fn create_api_key(&self, org_id: uuid::Uuid, name: &str, prefix: &str, hash: &str) -> DbResult<ApiKeyRow> {
        let id = uuid::Uuid::new_v4();
        sqlx::query(
            "INSERT INTO api_keys (id, org_id, name, key_prefix, key_hash) VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(id.to_string())
        .bind(org_id.to_string())
        .bind(name)
        .bind(prefix)
        .bind(hash)
        .execute(&self.pool)
        .await?;

        Ok(ApiKeyRow {
            id, org_id, key_prefix: prefix.to_string(), name: name.to_string(),
            created_at: chrono::Utc::now(), last_used_at: None,
        })
    }

    async fn revoke_api_key(&self, key_id: uuid::Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM api_keys WHERE id = ?1")
            .bind(key_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn find_key_by_prefix(&self, prefix: &str) -> DbResult<Option<ApiKeyMetaRow>> {
        let row = sqlx::query(
            "SELECT id, org_id, key_hash FROM api_keys WHERE key_prefix = ?1",
        )
        .bind(prefix)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| ApiKeyMetaRow {
            id: uuid::Uuid::parse_str(r.get::<&str, _>("id")).unwrap_or_default(),
            org_id: uuid::Uuid::parse_str(r.get::<&str, _>("org_id")).unwrap_or_default(),
            key_hash: r.get("key_hash"),
        }))
    }

    async fn search(&self, query: &str, limit: i64) -> DbResult<Vec<SearchResultRow>> {
        // Use FTS5 for search
        let rows = sqlx::query(
            r#"
            SELECT
                atc.conversation_id,
                atc.user_prompt,
                atc.model,
                atc.agent,
                atc.tool_name,
                atc.started_at,
                'fts' AS match_field
            FROM atc_fts fts
            JOIN agent_tool_calls atc ON atc.id = fts.rowid
            WHERE atc_fts MATCH ?1
            ORDER BY fts.rank
            LIMIT ?2
            "#,
        )
        .bind(query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| SearchResultRow {
            conversation_id: r.get("conversation_id"),
            user_prompt: r.get("user_prompt"),
            model: r.get("model"),
            agent: r.get("agent"),
            tool_name: r.get("tool_name"),
            started_at: r.try_get::<&str, _>("started_at").ok().map(parse_dt),
            match_field: r.get("match_field"),
        }).collect())
    }

    async fn migrate(&self) -> DbResult<()> {
        self.run_migrations().await
    }

    async fn health_check(&self) -> DbResult<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn sqlite_row_to_tool_call(r: &sqlx::sqlite::SqliteRow) -> ToolCallRow {
    ToolCallRow {
        id: r.get("id"),
        event_id: uuid::Uuid::parse_str(r.get::<&str, _>("event_id")).unwrap_or_default(),
        task_id: r.get("task_id"),
        repo: r.get("repo"),
        branch: r.get("branch"),
        ide: r.get("ide"),
        agent: r.get("agent"),
        skill: r.get("skill"),
        mcp_server: r.get("mcp_server"),
        tool_name: r.get("tool_name"),
        started_at: parse_dt(r.get("started_at")),
        ended_at: parse_dt(r.get("ended_at")),
        duration_ms: r.get("duration_ms"),
        ok: r.get("ok"),
        error: r.get("error"),
        request_bytes: r.get("request_bytes"),
        response_bytes: r.get("response_bytes"),
        estimated_input_tokens: r.get("estimated_input_tokens"),
        estimated_output_tokens: r.get("estimated_output_tokens"),
        estimated_total_tokens: r.get("estimated_total_tokens"),
        request_sha256: r.get("request_sha256"),
        response_sha256: r.get("response_sha256"),
        metadata: r.get::<String, _>("metadata").parse().unwrap_or_default(),
        created_at: parse_dt(r.get("created_at")),
        model: r.get("model"),
        cached_tokens: r.get("cached_tokens"),
        conversation_id: r.get("conversation_id"),
        client_ip: r.get("client_ip"),
        user_agent: r.get("user_agent"),
        user_prompt: r.get("user_prompt"),
        tool_arguments: r.get::<Option<String>, _>("tool_arguments").and_then(|s| serde_json::from_str(&s).ok()),
        tool_result: r.get("tool_result"),
        reasoning_tokens: r.get("reasoning_tokens"),
        finish_reason: r.get("finish_reason"),
        request_max_tokens: r.get("request_max_tokens"),
        request_temperature: r.get("request_temperature"),
        llm_system: r.get("llm_system"),
        trace_id: r.get("trace_id"),
        span_id: r.get("span_id"),
        parent_span_id: r.get("parent_span_id"),
        tool_call_id: r.get("tool_call_id"),
        usd_cost: r.get("usd_cost"),
        billing_model: r.get("billing_model"),
    }
}
