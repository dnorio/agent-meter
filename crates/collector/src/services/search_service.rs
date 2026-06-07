use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SearchResult {
    pub conversation_id: String,
    pub user_prompt: Option<String>,
    pub model: Option<String>,
    pub agent: Option<String>,
    pub tool_name: Option<String>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub match_field: String,
}

/// Full-text-ish search across agent_tool_calls.
/// Searches: user_prompt, tool_name, model, agent, conversation_id, skill, mcp_server.
pub async fn search(pool: &PgPool, query: &str, limit: i64) -> Result<Vec<SearchResult>, sqlx::Error> {
    let pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));

    let rows = sqlx::query_as::<_, SearchResult>(
        r#"
        SELECT DISTINCT ON (conversation_id)
            conversation_id,
            user_prompt,
            model,
            agent,
            tool_name,
            started_at,
            CASE
                WHEN user_prompt ILIKE $1 THEN 'prompt'
                WHEN tool_name ILIKE $1 THEN 'tool'
                WHEN model ILIKE $1 THEN 'model'
                WHEN agent ILIKE $1 THEN 'agent'
                WHEN conversation_id ILIKE $1 THEN 'conversation'
                WHEN skill ILIKE $1 THEN 'skill'
                WHEN mcp_server ILIKE $1 THEN 'mcp_server'
                ELSE 'other'
            END AS match_field
        FROM agent_tool_calls
        WHERE user_prompt ILIKE $1
           OR tool_name ILIKE $1
           OR model ILIKE $1
           OR agent ILIKE $1
           OR conversation_id ILIKE $1
           OR skill ILIKE $1
           OR mcp_server ILIKE $1
        ORDER BY conversation_id, started_at DESC
        LIMIT $2
        "#,
    )
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
