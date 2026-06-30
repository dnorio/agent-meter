//! Maps an incoming `ToolCallEvent` into a backend-agnostic `InsertToolCall`,
//! estimating tokens when explicit counts are absent. Persistence itself goes
//! through the `Database` trait (Postgres or SQLite), keeping this layer free
//! of any SQL dialect.

use agent_meter_db::models::InsertToolCall;

use crate::models::event::ToolCallEvent;
use crate::services::token_estimator;

/// Convert a raw ingest event into the database insert struct.
pub fn to_insert(event: ToolCallEvent) -> InsertToolCall {
    let duration_ms = (event.ended_at - event.started_at).num_milliseconds() as i32;

    let estimated_input =
        token_estimator::estimate_input_tokens(event.request_bytes, event.estimated_input_tokens);
    let estimated_output =
        token_estimator::estimate_output_tokens(event.response_bytes, event.estimated_output_tokens);
    let estimated_total = token_estimator::estimate_total(estimated_input, estimated_output);

    InsertToolCall {
        event_id: event.event_id,
        task_id: event.task_id,
        repo: event.repo,
        branch: event.branch,
        ide: event.ide,
        agent: event.agent,
        skill: event.skill,
        mcp_server: event.mcp_server,
        tool_name: event.tool_name,
        started_at: event.started_at,
        ended_at: event.ended_at,
        duration_ms,
        ok: event.ok,
        error: event.error,
        request_bytes: event.request_bytes,
        response_bytes: event.response_bytes,
        estimated_input_tokens: estimated_input,
        estimated_output_tokens: estimated_output,
        estimated_total_tokens: estimated_total,
        request_sha256: event.request_sha256,
        response_sha256: event.response_sha256,
        metadata: event
            .metadata
            .unwrap_or(serde_json::Value::Object(Default::default())),
        model: event.model,
        cached_tokens: event.cached_tokens,
        conversation_id: event.conversation_id,
        client_ip: event.client_ip,
        user_agent: event.user_agent,
        user_prompt: event.user_prompt,
        tool_arguments: event.tool_arguments,
        tool_result: event.tool_result,
        reasoning_tokens: event.reasoning_tokens,
        finish_reason: event.finish_reason,
        request_max_tokens: event.request_max_tokens,
        request_temperature: event.request_temperature,
        llm_system: event.llm_system,
        trace_id: event.trace_id,
        span_id: event.span_id,
        parent_span_id: event.parent_span_id,
        tool_call_id: event.tool_call_id,
    }
}
