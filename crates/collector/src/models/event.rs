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
}

fn default_uuid() -> Uuid {
    Uuid::new_v4()
}
