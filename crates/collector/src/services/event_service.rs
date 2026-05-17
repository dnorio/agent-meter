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

    let span = tracing::info_span!(
        "agent.tool_call",
        event_id = %event.event_id,
        task_id = event.task_id.as_deref().unwrap_or(""),
        repo = event.repo.as_deref().unwrap_or(""),
        branch = event.branch.as_deref().unwrap_or(""),
        ide = event.ide.as_deref().unwrap_or(""),
        agent = event.agent.as_deref().unwrap_or(""),
        skill = event.skill.as_deref().unwrap_or(""),
        mcp_server = event.mcp_server.as_deref().unwrap_or(""),
        tool_name = %event.tool_name,
        duration_ms = duration_ms,
        ok = event.ok,
        request_bytes = event.request_bytes.unwrap_or(0),
        response_bytes = event.response_bytes.unwrap_or(0),
        input_tokens = estimated_input.unwrap_or(0),
        output_tokens = estimated_output.unwrap_or(0),
        total_tokens = estimated_total.unwrap_or(0),
    );
    let _guard = span.enter();

    let event_id = event.event_id;
    let metadata = event.metadata.unwrap_or(serde_json::Value::Object(Default::default()));

    let row = sqlx::query_as::<_, AgentToolCall>(
        r#"
        INSERT INTO agent_tool_calls (
            event_id, task_id, repo, branch, ide, agent, skill,
            mcp_server, tool_name, started_at, ended_at, duration_ms,
            ok, error, request_bytes, response_bytes,
            estimated_input_tokens, estimated_output_tokens, estimated_total_tokens,
            request_sha256, response_sha256, metadata,
            model, cached_tokens, conversation_id, client_ip, user_agent
        ) VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
            $13, $14, $15, $16, $17, $18, $19, $20, $21, $22,
            $23, $24, $25, $26, $27
        )
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
    .bind(&event.model)
    .bind(event.cached_tokens)
    .bind(&event.conversation_id)
    .bind(&event.client_ip)
    .bind(&event.user_agent)
    .fetch_one(pool)
    .await?;

    drop(_guard);
    Ok(row)
}
