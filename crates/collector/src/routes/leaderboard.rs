//! T-323 — Quickstart + Leaderboard + VS pages & API

use axum::{
    extract::{Query, State},
    response::Html,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::errors::AppError;

const QUICKSTART_HTML: &str = include_str!("../../ui/quickstart.html");
const LEADERBOARD_HTML: &str = include_str!("../../ui/leaderboard.html");
const VS_HTML: &str = include_str!("../../ui/vs.html");

// --- Pages ---

async fn quickstart_page() -> Html<&'static str> {
    Html(QUICKSTART_HTML)
}
async fn leaderboard_page() -> Html<&'static str> {
    Html(LEADERBOARD_HTML)
}
async fn vs_page() -> Html<&'static str> {
    Html(VS_HTML)
}

// --- API ---

#[derive(Deserialize)]
struct LeaderboardQuery {
    from: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize, sqlx::FromRow)]
struct AgentEntry {
    agent: String,
    events: i64,
    usd_cost: f64,
}

#[derive(Serialize, sqlx::FromRow)]
struct IdeEntry {
    ide: String,
    events: i64,
    usd_cost: f64,
}

#[derive(Serialize, sqlx::FromRow)]
struct ModelEntry {
    model: String,
    events: i64,
    usd_cost: f64,
}

async fn leaderboard_agents(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<Json<Vec<AgentEntry>>, AppError> {
    let from = q.from.unwrap_or_else(|| "2000-01-01".into());
    let limit = q.limit.unwrap_or(20);
    let rows = sqlx::query_as::<_, AgentEntry>(
        "SELECT COALESCE(agent, 'unknown') as agent, \
         COUNT(*)::int8 as events, \
         COALESCE(SUM(usd_cost), 0)::float8 as usd_cost \
         FROM agent_tool_calls WHERE created_at >= $1::timestamptz \
         GROUP BY agent ORDER BY events DESC LIMIT $2",
    )
    .bind(&from)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn leaderboard_ides(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<Json<Vec<IdeEntry>>, AppError> {
    let from = q.from.unwrap_or_else(|| "2000-01-01".into());
    let limit = q.limit.unwrap_or(20);
    let rows = sqlx::query_as::<_, IdeEntry>(
        "SELECT COALESCE(ide, 'unknown') as ide, \
         COUNT(*)::int8 as events, \
         COALESCE(SUM(usd_cost), 0)::float8 as usd_cost \
         FROM agent_tool_calls WHERE created_at >= $1::timestamptz \
         GROUP BY ide ORDER BY events DESC LIMIT $2",
    )
    .bind(&from)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn leaderboard_models(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<Json<Vec<ModelEntry>>, AppError> {
    let from = q.from.unwrap_or_else(|| "2000-01-01".into());
    let limit = q.limit.unwrap_or(20);
    let rows = sqlx::query_as::<_, ModelEntry>(
        "SELECT COALESCE(model, 'unknown') as model, \
         COUNT(*)::int8 as events, \
         COALESCE(SUM(usd_cost), 0)::float8 as usd_cost \
         FROM agent_tool_calls WHERE created_at >= $1::timestamptz \
         GROUP BY model ORDER BY events DESC LIMIT $2",
    )
    .bind(&from)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/quickstart", get(quickstart_page))
        .route("/leaderboard", get(leaderboard_page))
        .route("/vs", get(vs_page))
        .route("/api/leaderboard/agents", get(leaderboard_agents))
        .route("/api/leaderboard/ides", get(leaderboard_ides))
        .route("/api/leaderboard/models", get(leaderboard_models))
}
