use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ToolCallEvent {
    #[serde(default = "default_uuid")]
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
    pub ok: bool,
    pub error: Option<String>,
    pub request_bytes: Option<i32>,
    pub response_bytes: Option<i32>,
    pub estimated_input_tokens: Option<i32>,
    pub estimated_output_tokens: Option<i32>,
    pub request_sha256: Option<String>,
    pub response_sha256: Option<String>,
    pub metadata: Option<serde_json::Value>,
    // Enriched fields (T-239)
    pub model: Option<String>,
    pub cached_tokens: Option<i32>,
    pub conversation_id: Option<String>,
    pub client_ip: Option<String>,
    pub user_agent: Option<String>,
    pub user_prompt: Option<String>,
    /// JSON arguments passed to the tool (from mcp-wrapper or OTLP)
    pub tool_arguments: Option<serde_json::Value>,
    /// Tool output/result content, truncated at 8 KB (from mcp-wrapper)
    pub tool_result: Option<String>,
    // T-332: deep telemetry
    /// Reasoning/thinking tokens (o1, o3, Claude extended thinking — billed separately)
    pub reasoning_tokens: Option<i32>,
    /// Why the LLM stopped: "stop", "length", "tool_calls", "content_filter"
    pub finish_reason: Option<String>,
    /// gen_ai.request.max_tokens — max completion tokens configured
    pub request_max_tokens: Option<i32>,
    /// gen_ai.request.temperature
    pub request_temperature: Option<f64>,
    /// gen_ai.system — "openai", "anthropic", "google_genai", etc.
    pub llm_system: Option<String>,
    /// OTLP traceId (hex) — groups all spans in one inference round
    pub trace_id: Option<String>,
    /// OTLP spanId (hex)
    pub span_id: Option<String>,
    /// OTLP parentSpanId (hex) — links tool call to its LLM parent span
    pub parent_span_id: Option<String>,
    /// LLM-assigned tool call ID (gen_ai.tool.call.id or JSON-RPC id)
    pub tool_call_id: Option<String>,
}

fn default_uuid() -> Uuid {
    Uuid::new_v4()
}
