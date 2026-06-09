//! Domain models returned by the Database trait.
//! These are backend-agnostic — no `sqlx::FromRow` here.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

// ── Tool Call (full row) ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ToolCallRow {
    pub id: i64,
    pub event_id: Uuid,
    pub task_id: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub ide: Option<String>,
    pub agent: Option<String>,
    pub skill: Option<String>,
    pub mcp_server: Option<String>,
    pub tool_name: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_ms: i32,
    pub ok: bool,
    pub error: Option<String>,
    pub request_bytes: Option<i32>,
    pub response_bytes: Option<i32>,
    pub estimated_input_tokens: Option<i32>,
    pub estimated_output_tokens: Option<i32>,
    pub estimated_total_tokens: Option<i32>,
    pub request_sha256: Option<String>,
    pub response_sha256: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub model: Option<String>,
    pub cached_tokens: Option<i32>,
    pub conversation_id: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub user_prompt: Option<String>,
    pub tool_arguments: Option<serde_json::Value>,
    pub tool_result: Option<String>,
    pub reasoning_tokens: Option<i32>,
    pub finish_reason: Option<String>,
    pub request_max_tokens: Option<i32>,
    pub request_temperature: Option<f64>,
    pub llm_system: Option<String>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub parent_span_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub usd_cost: Option<f64>,
    pub billing_model: String,
}

// ── Event Feed (compact for lists) ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EventFeedRow {
    pub event_id: Uuid,
    pub tool_name: String,
    pub model: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_ms: i32,
    pub ok: bool,
    pub estimated_input_tokens: Option<i32>,
    pub estimated_output_tokens: Option<i32>,
    pub cached_tokens: Option<i32>,
    pub agent: Option<String>,
    pub ide: Option<String>,
    pub mcp_server: Option<String>,
    pub conversation_id: Option<String>,
    pub client_ip: Option<String>,
    pub user_prompt: Option<String>,
    pub tool_arguments: Option<serde_json::Value>,
    pub tool_result: Option<String>,
}

// ── Report aggregates ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct TopToolRow {
    pub mcp_server: Option<String>,
    pub tool_name: String,
    pub calls: i64,
    pub total_estimated_tokens: Option<i64>,
    pub avg_duration_ms: Option<f64>,
    pub errors: i64,
    pub avg_response_bytes: Option<f64>,
    pub top_model: Option<String>,
    pub cached_tokens_total: Option<i64>,
    pub avg_input_tokens: Option<f64>,
    pub avg_output_tokens: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopAgentRow {
    pub agent: String,
    pub calls: i64,
    pub total_tokens: Option<i64>,
    pub total_usd_cost: Option<f64>,
    pub errors: i64,
    pub conversations: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopMcpServerRow {
    pub mcp_server: String,
    pub calls: i64,
    pub total_estimated_tokens: Option<i64>,
    pub avg_response_bytes: Option<f64>,
    pub error_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeBreakdownRow {
    pub ide: String,
    pub calls: i64,
    pub total_estimated_tokens: Option<i64>,
    pub errors: i64,
    pub llm_calls: i64,
    pub tool_calls_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPatternRow {
    pub error: String,
    pub occurrences: i64,
    pub tool_name: Option<String>,
    pub model: Option<String>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CostBucketRow {
    pub bucket: DateTime<Utc>,
    pub total_usd: Option<f64>,
    pub calls: i64,
}

// ── Conversations ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct TopTaskRow {
    pub task_id: String,
    pub tool_calls: i64,
    pub total_estimated_tokens: Option<i64>,
    pub total_duration_ms: Option<i64>,
    pub errors: i64,
    pub distinct_tools: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallsBucketRow {
    pub bucket: DateTime<Utc>,
    pub calls: i64,
    pub errors: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LeaderboardEntry {
    pub name: String,
    pub events: i64,
    pub usd_cost: f64,
}

// ── Conversations ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
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

// ── Cost ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CostKpisRow {
    pub total_usd: f64,
    pub total_credits: f64,
    pub total_events: i64,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub avg_usd_per_event: f64,
    pub burn_rate_usd_per_hour: f64,
    pub avg_duration_ms: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelCostRow {
    pub model: Option<String>,
    pub events: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub usd_cost: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CostByDayRow {
    pub day: DateTime<Utc>,
    pub usd_cost: f64,
    pub events: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BillingModelBreakdownRow {
    pub billing_model: String,
    pub events: i64,
    pub usd_cost: f64,
    pub credits: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CostSummaryResult {
    pub kpis: CostKpisRow,
    pub by_model: Vec<ModelCostRow>,
    pub by_day: Vec<CostByDayRow>,
    pub by_billing_model: Vec<BillingModelBreakdownRow>,
}

// ── Organizations ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct OrgRow {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub plan: String,
    pub created_at: DateTime<Utc>,
}

// ── API Keys ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub key_prefix: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Minimal key metadata for auth lookup.
#[derive(Debug, Clone)]
pub struct ApiKeyMetaRow {
    pub id: Uuid,
    pub org_id: Uuid,
    pub key_hash: String,
}

// ── Search ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SearchResultRow {
    pub conversation_id: String,
    pub user_prompt: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub tool_name: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub match_field: String,
}

// ── Insert event payload ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InsertToolCall {
    pub event_id: Uuid,
    pub task_id: Option<String>,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub ide: Option<String>,
    pub agent: Option<String>,
    pub skill: Option<String>,
    pub mcp_server: Option<String>,
    pub tool_name: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_ms: i32,
    pub ok: bool,
    pub error: Option<String>,
    pub request_bytes: Option<i32>,
    pub response_bytes: Option<i32>,
    pub estimated_input_tokens: Option<i32>,
    pub estimated_output_tokens: Option<i32>,
    pub estimated_total_tokens: Option<i32>,
    pub request_sha256: Option<String>,
    pub response_sha256: Option<String>,
    pub metadata: serde_json::Value,
    pub model: Option<String>,
    pub cached_tokens: Option<i32>,
    pub conversation_id: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub user_prompt: Option<String>,
    pub tool_arguments: Option<serde_json::Value>,
    pub tool_result: Option<String>,
    pub reasoning_tokens: Option<i32>,
    pub finish_reason: Option<String>,
    pub request_max_tokens: Option<i32>,
    pub request_temperature: Option<f64>,
    pub llm_system: Option<String>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub parent_span_id: Option<String>,
    pub tool_call_id: Option<String>,
}
