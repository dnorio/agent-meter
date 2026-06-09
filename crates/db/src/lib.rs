//! agent-meter-db — Database abstraction layer.
//!
//! Defines the `Database` trait and provides Postgres + SQLite implementations.
//! Future backends (DuckDB, etc.) implement the same trait.

pub mod models;
pub mod params;
pub mod postgres;
pub mod sqlite;

use async_trait::async_trait;
use models::*;
use params::*;

pub use postgres::PostgresDb;
pub use sqlite::SqliteDb;

/// Result type for database operations.
pub type DbResult<T> = Result<T, DbError>;

/// Database errors.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("database error: {0}")]
    Internal(String),
}

impl From<sqlx::Error> for DbError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => DbError::NotFound,
            other => DbError::Internal(other.to_string()),
        }
    }
}

/// The core database trait. All services depend on this, not on `PgPool` directly.
#[async_trait]
pub trait Database: Send + Sync + 'static {
    // ── Events ──────────────────────────────────────────────────────────────
    async fn insert_tool_call(&self, event: &InsertToolCall) -> DbResult<ToolCallRow>;
    async fn query_events(&self, params: &EventQuery) -> DbResult<Vec<EventFeedRow>>;

    // ── Reports ─────────────────────────────────────────────────────────────
    async fn top_tools(&self, params: &ReportQuery) -> DbResult<Vec<TopToolRow>>;
    async fn top_agents(&self, params: &ReportQuery) -> DbResult<Vec<TopAgentRow>>;
    async fn top_mcp_servers(&self, params: &ReportQuery) -> DbResult<Vec<TopMcpServerRow>>;
    async fn top_tasks(&self, params: &ReportQuery) -> DbResult<Vec<TopTaskRow>>;
    async fn ide_breakdown(&self, params: &ReportQuery) -> DbResult<Vec<IdeBreakdownRow>>;
    async fn error_patterns(&self, params: &ReportQuery) -> DbResult<Vec<ErrorPatternRow>>;
    async fn cost_over_time(&self, params: &ReportQuery) -> DbResult<Vec<CostBucketRow>>;
    async fn calls_over_time(&self, params: &ReportQuery, bucket: &str) -> DbResult<Vec<CallsBucketRow>>;
    async fn distinct_models(&self) -> DbResult<Vec<String>>;

    // ── Leaderboard ─────────────────────────────────────────────────────────
    async fn leaderboard_agents(&self, from: &str, limit: i64) -> DbResult<Vec<LeaderboardEntry>>;
    async fn leaderboard_ides(&self, from: &str, limit: i64) -> DbResult<Vec<LeaderboardEntry>>;
    async fn leaderboard_models(&self, from: &str, limit: i64) -> DbResult<Vec<LeaderboardEntry>>;

    // ── Conversations ───────────────────────────────────────────────────────
    async fn list_conversations(&self, params: &ConversationQuery) -> DbResult<Vec<ConversationRow>>;
    async fn conversation_detail(&self, conversation_id: &str) -> DbResult<Vec<ToolCallRow>>;

    // ── Cost ────────────────────────────────────────────────────────────────
    async fn cost_summary(&self, params: &CostQuery) -> DbResult<CostSummaryResult>;

    // ── Organizations ───────────────────────────────────────────────────────
    async fn list_orgs(&self) -> DbResult<Vec<OrgRow>>;
    async fn find_org_by_slug(&self, slug: &str) -> DbResult<OrgRow>;

    // ── API Keys ────────────────────────────────────────────────────────────
    async fn list_api_keys(&self, org_id: uuid::Uuid) -> DbResult<Vec<ApiKeyRow>>;
    async fn create_api_key(&self, org_id: uuid::Uuid, name: &str, prefix: &str, hash: &str) -> DbResult<ApiKeyRow>;
    async fn revoke_api_key(&self, key_id: uuid::Uuid) -> DbResult<()>;
    async fn find_key_by_prefix(&self, prefix: &str) -> DbResult<Option<ApiKeyMetaRow>>;

    // ── Search ──────────────────────────────────────────────────────────────
    async fn search(&self, query: &str, limit: i64) -> DbResult<Vec<SearchResultRow>>;

    // ── Migrations ──────────────────────────────────────────────────────────
    async fn migrate(&self) -> DbResult<()>;

    // ── Health ──────────────────────────────────────────────────────────────
    async fn health_check(&self) -> DbResult<()>;
}
