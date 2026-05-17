use sqlx::PgPool;

use crate::errors::AppError;

pub async fn start_task(
    pool: &PgPool,
    task_id: &str,
    repo: Option<&str>,
    branch: Option<&str>,
    ide: Option<&str>,
    agent: Option<&str>,
    skill: Option<&str>,
) -> Result<serde_json::Value, AppError> {
    let row = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>, serde_json::Value)>(
        r#"
        INSERT INTO agent_tasks (task_id, repo, branch, ide, agent, skill)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (task_id) DO UPDATE
            SET started_at = now(),
                ended_at = NULL,
                repo = COALESCE($2, agent_tasks.repo),
                branch = COALESCE($3, agent_tasks.branch),
                ide = COALESCE($4, agent_tasks.ide),
                agent = COALESCE($5, agent_tasks.agent),
                skill = COALESCE($6, agent_tasks.skill)
        RETURNING id, task_id, repo, branch, ide, agent, skill, started_at, ended_at, metadata
        "#,
    )
    .bind(task_id)
    .bind(repo)
    .bind(branch)
    .bind(ide)
    .bind(agent)
    .bind(skill)
    .fetch_one(pool)
    .await?;

    Ok(serde_json::json!({
        "id": row.0,
        "task_id": row.1,
        "repo": row.2,
        "branch": row.3,
        "ide": row.4,
        "agent": row.5,
        "skill": row.6,
        "started_at": row.7,
        "ended_at": row.8,
        "metadata": row.9,
    }))
}

pub async fn end_task(
    pool: &PgPool,
    task_id: &str,
) -> Result<serde_json::Value, AppError> {
    let row = sqlx::query_as::<_, (i64, String, Option<chrono::DateTime<chrono::Utc>>)>(
        r#"
        UPDATE agent_tasks
        SET ended_at = now()
        WHERE task_id = $1
        RETURNING id, task_id, ended_at
        "#,
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some((id, tid, ended_at)) => Ok(serde_json::json!({
            "id": id,
            "task_id": tid,
            "ended_at": ended_at,
        })),
        None => Err(AppError::NotFound(format!("task {task_id} not found"))),
    }
}

pub async fn list_tasks(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<serde_json::Value>, AppError> {
    #[derive(sqlx::FromRow)]
    struct TaskRow {
        id: i64,
        task_id: String,
        repo: Option<String>,
        branch: Option<String>,
        ide: Option<String>,
        agent: Option<String>,
        skill: Option<String>,
        started_at: chrono::DateTime<chrono::Utc>,
        ended_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let rows = sqlx::query_as::<_, TaskRow>(
        r#"
        SELECT id, task_id, repo, branch, ide, agent, skill, started_at, ended_at
        FROM agent_tasks
        ORDER BY started_at DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let tasks: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            let duration = r.ended_at.map(|e| (e - r.started_at).num_seconds());
            serde_json::json!({
                "id": r.id,
                "task_id": r.task_id,
                "repo": r.repo,
                "branch": r.branch,
                "ide": r.ide,
                "agent": r.agent,
                "skill": r.skill,
                "started_at": r.started_at,
                "ended_at": r.ended_at,
                "duration_secs": duration,
            })
        })
        .collect();

    Ok(tasks)
}
