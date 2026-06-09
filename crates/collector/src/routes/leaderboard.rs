//! T-323 — Quickstart + Leaderboard + VS pages & API

use axum::{
    extract::{Query, State},
    http::header,
    response::{Html, IntoResponse},
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

#[derive(Serialize)]
struct LeaderboardAll {
    agents: Vec<AgentEntry>,
    ides: Vec<IdeEntry>,
    models: Vec<ModelEntry>,
}

async fn leaderboard_agents(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<impl IntoResponse, AppError> {
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
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(rows),
    ))
}

async fn leaderboard_ides(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<impl IntoResponse, AppError> {
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
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(rows),
    ))
}

async fn leaderboard_models(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<impl IntoResponse, AppError> {
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
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(rows),
    ))
}

/// Single endpoint that returns all 3 leaderboards in one request (saves 2 round-trips)
async fn leaderboard_all(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<impl IntoResponse, AppError> {
    let from = q.from.unwrap_or_else(|| "2000-01-01".into());
    let limit = q.limit.unwrap_or(20);

    let agents_fut = sqlx::query_as::<_, AgentEntry>(
        "SELECT COALESCE(agent, 'unknown') as agent, \
         COUNT(*)::int8 as events, \
         COALESCE(SUM(usd_cost), 0)::float8 as usd_cost \
         FROM agent_tool_calls WHERE created_at >= $1::timestamptz \
         GROUP BY agent ORDER BY events DESC LIMIT $2",
    )
    .bind(&from)
    .bind(limit)
    .fetch_all(&state.pool);

    let ides_fut = sqlx::query_as::<_, IdeEntry>(
        "SELECT COALESCE(ide, 'unknown') as ide, \
         COUNT(*)::int8 as events, \
         COALESCE(SUM(usd_cost), 0)::float8 as usd_cost \
         FROM agent_tool_calls WHERE created_at >= $1::timestamptz \
         GROUP BY ide ORDER BY events DESC LIMIT $2",
    )
    .bind(&from)
    .bind(limit)
    .fetch_all(&state.pool);

    let models_fut = sqlx::query_as::<_, ModelEntry>(
        "SELECT COALESCE(model, 'unknown') as model, \
         COUNT(*)::int8 as events, \
         COALESCE(SUM(usd_cost), 0)::float8 as usd_cost \
         FROM agent_tool_calls WHERE created_at >= $1::timestamptz \
         GROUP BY model ORDER BY events DESC LIMIT $2",
    )
    .bind(&from)
    .bind(limit)
    .fetch_all(&state.pool);

    let (agents, ides, models) = tokio::join!(agents_fut, ides_fut, models_fut);

    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(LeaderboardAll {
            agents: agents?,
            ides: ides?,
            models: models?,
        }),
    ))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/quickstart", get(quickstart_page))
        .route("/leaderboard", get(leaderboard_page))
        .route("/vs", get(vs_page))
        .route("/api/leaderboard/agents", get(leaderboard_agents))
        .route("/api/leaderboard/ides", get(leaderboard_ides))
        .route("/api/leaderboard/models", get(leaderboard_models))
        .route("/api/leaderboard/all", get(leaderboard_all))
}
