//! T-323 — Quickstart + Leaderboard + VS pages & API
//! Uses Database trait for all queries (no inline SQL).

use axum::{
    extract::{Query, State},
    http::header,
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use agent_meter_db::models::LeaderboardEntry;

use crate::app::AppState;
use crate::errors::AppError;

const QUICKSTART_HTML: &str = include_str!("../../ui/quickstart.html");
const LEADERBOARD_HTML: &str = include_str!("../../ui/leaderboard.html");
const VS_HTML: &str = include_str!("../../ui/vs.html");

// ── Pages ───────────────────────────────────────────────────────────────────

async fn quickstart_page() -> Html<&'static str> {
    Html(QUICKSTART_HTML)
}
async fn leaderboard_page() -> Html<&'static str> {
    Html(LEADERBOARD_HTML)
}
async fn vs_page() -> Html<&'static str> {
    Html(VS_HTML)
}

// ── API ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LeaderboardQuery {
    from: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct LeaderboardAll {
    agents: Vec<LeaderboardEntry>,
    ides: Vec<LeaderboardEntry>,
    models: Vec<LeaderboardEntry>,
}

async fn leaderboard_agents(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<impl IntoResponse, AppError> {
    let from = q.from.unwrap_or_else(|| "2000-01-01".into());
    let limit = q.limit.unwrap_or(20);
    let rows = state.db.leaderboard_agents(&from, limit).await?;
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
    let rows = state.db.leaderboard_ides(&from, limit).await?;
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
    let rows = state.db.leaderboard_models(&from, limit).await?;
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(rows),
    ))
}

/// Single endpoint that returns all 3 leaderboards in one request (saves 2 round-trips).
async fn leaderboard_all(
    State(state): State<AppState>,
    Query(q): Query<LeaderboardQuery>,
) -> Result<impl IntoResponse, AppError> {
    let from = q.from.unwrap_or_else(|| "2000-01-01".into());
    let limit = q.limit.unwrap_or(20);

    let (agents, ides, models) = tokio::join!(
        state.db.leaderboard_agents(&from, limit),
        state.db.leaderboard_ides(&from, limit),
        state.db.leaderboard_models(&from, limit),
    );

    Ok((
        [(header::CACHE_CONTROL, "public, max-age=300")],
        Json(LeaderboardAll {
            agents: agents?,
            ides: ides?,
            models: models?,
        }),
    ))
}

// ── Router ──────────────────────────────────────────────────────────────────

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/quickstart", get(quickstart_page))
        .route("/leaderboard", get(leaderboard_page))
        .route("/vs", get(vs_page))
        .route("/vs/{competitor}", get(vs_page))
        .route("/api/leaderboard/agents", get(leaderboard_agents))
        .route("/api/leaderboard/ides", get(leaderboard_ides))
        .route("/api/leaderboard/models", get(leaderboard_models))
        .route("/api/leaderboard/all", get(leaderboard_all))
}
