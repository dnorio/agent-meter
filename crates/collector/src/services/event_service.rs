use sqlx::PgPool;

use crate::errors::AppError;
use crate::models::event::ToolCallEvent;
use crate::models::tool_call::AgentToolCall;
use crate::services::token_estimator;

pub async fn insert_tool_call(
    pool: &PgPool,
    event: ToolCallEvent,
) -> Result<AgentToolCall, AppError> {
    let duration_ms = (event.ended_at - event.started_at)
        .num_milliseconds() as i32;

    let estimated_input = token_estimator::estimate_input_tokens(
        event.request_bytes,
        event.estimated_input_tokens,
    );
    let estimated_output = token_estimator::estimate_output_tokens(
        event.response_bytes,
        event.estimated_output_tokens,
    );
    let estimated_total = token_estimator::estimate_total(estimated_input, estimated_output);

    let event_id = event.event_id;
    let metadata = event.metadata.unwrap_or(serde_json::Value::Object(Default::default()));

    let row = sqlx::query_as::<_, AgentToolCall>(
        r#"
        INSERT INTO agent_tool_calls (
            event_id, task_id, repo, branch, ide, agent, skill,
            mcp_server, tool_name, started_at, ended_at, duration_ms,
            ok, error, request_bytes, response_bytes,
            estimated_input_tokens, estimated_output_tokens, estimated_total_tokens,
            request_sha256, response_sha256, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22)
        RETURNING *
        "#,
    )
    .bind(event_id)
    .bind(&event.task_id)
    .bind(&event.repo)
    .bind(&event.branch)
    .bind(&event.ide)
    .bind(&event.agent)
    .bind(&event.skill)
    .bind(&event.mcp_server)
    .bind(&event.tool_name)
    .bind(event.started_at)
    .bind(event.ended_at)
    .bind(duration_ms)
    .bind(event.ok)
    .bind(&event.error)
    .bind(event.request_bytes)
    .bind(event.response_bytes)
    .bind(estimated_input)
    .bind(estimated_output)
    .bind(estimated_total)
    .bind(&event.request_sha256)
    .bind(&event.response_sha256)
    .bind(&metadata)
    .fetch_one(pool)
    .await?;

    Ok(row)
}
