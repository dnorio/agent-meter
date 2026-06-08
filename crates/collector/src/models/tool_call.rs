use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize)]
pub struct AgentToolCall {
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
    // Enriched fields (T-239)
    pub model: Option<String>,
    pub cached_tokens: Option<i32>,
    pub conversation_id: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub user_prompt: Option<String>,
    // T-331: full agentic payload
    pub tool_arguments: Option<serde_json::Value>,
    pub tool_result: Option<String>,
    // T-332: deep telemetry
    pub reasoning_tokens: Option<i32>,
    pub finish_reason: Option<String>,
    pub request_max_tokens: Option<i32>,
    pub request_temperature: Option<f64>,
    pub llm_system: Option<String>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub parent_span_id: Option<String>,
    pub tool_call_id: Option<String>,
    // T-355: pre-computed cost
    pub usd_cost: Option<f64>,
    // T-357: billing model (token/credit/subscription)
    pub billing_model: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TopTool {
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

#[derive(Debug, Serialize, sqlx::FromRow)]
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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TopTask {
    pub task_id: String,
    pub tool_calls: i64,
    pub total_estimated_tokens: Option<i64>,
    pub total_duration_ms: Option<i64>,
    pub errors: i64,
    pub distinct_tools: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TopMcpServer {
    pub mcp_server: String,
    pub calls: i64,
    pub total_estimated_tokens: Option<i64>,
    pub avg_response_bytes: Option<f64>,
    pub error_rate: Option<f64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct IdeBreakdown {
    pub ide: String,
    pub calls: i64,
    pub total_estimated_tokens: Option<i64>,
    pub errors: i64,
    pub llm_calls: i64,
    pub tool_calls_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TopAgent {
    pub agent: String,
    pub calls: i64,
    pub total_tokens: Option<i64>,
    pub total_usd_cost: Option<f64>,
    pub errors: i64,
    pub conversations: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ErrorPattern {
    pub error: String,
    pub occurrences: i64,
    pub tool_name: Option<String>,
    pub model: Option<String>,
    pub last_seen: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CostBucket {
    pub bucket: DateTime<Utc>,
    pub total_usd: Option<f64>,
    pub calls: i64,
}
