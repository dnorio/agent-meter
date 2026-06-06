use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TimelineEvent {
    pub order: u32,
    pub tool_name: String,
    pub mcp_server: Option<String>,
    pub model: Option<String>,
    pub duration_ms: i32,
    pub tokens_in: Option<i32>,
    pub tokens_out: Option<i32>,
    pub usd_cost: f64,
    pub ok: bool,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    /// First 600 chars of the user prompt associated with the tool call (if any)
    pub user_prompt: Option<String>,
    /// Error message from span status if !ok
    pub error: Option<String>,
    /// JSON arguments passed to the tool (input)
    pub tool_arguments: Option<serde_json::Value>,
    /// Tool result/output content, truncated at 8 KB
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
}

#[derive(Debug, Serialize)]
pub struct ConversationTimeline {
    pub conversation_id: String,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub total_duration_ms: i64,
    pub total_tokens_in: i64,
    pub total_tokens_out: i64,
    pub total_usd_cost: f64,
    pub event_count: i64,
    pub error_count: i64,
    pub events: Vec<TimelineEvent>,
}
