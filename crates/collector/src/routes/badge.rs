//! Dynamic SVG badges (shields.io style) for README embedding.
//! Public route: GET /badge/cost.svg — shows total USD tracked this month.
//! GET /badge/events.svg — shows total events this month.

use axum::{
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};

use crate::app::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/badge/cost.svg", get(cost_badge))
        .route("/badge/events.svg", get(events_badge))
}

async fn cost_badge(State(state): State<AppState>) -> Response {
    let cost: f64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(usd_cost), 0)::float8 FROM agent_tool_calls WHERE started_at > date_trunc('month', now())"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0.0);

    let value = if cost < 1.0 {
        format!("${:.2}", cost)
    } else {
        format!("${:.0}", cost)
    };

    render_badge("AI cost this month", &value, "#1e90ff")
}

async fn events_badge(State(state): State<AppState>) -> Response {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM agent_tool_calls WHERE started_at > date_trunc('month', now())"
    )
    .fetch_one(&state.pool)
    .await
    .unwrap_or(0);

    let value = if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        format!("{}", count)
    };

    render_badge("events this month", &value, "#28a745")
}

fn render_badge(label: &str, value: &str, color: &str) -> Response {
    // Approximate character widths (shields.io compatible proportions)
    let label_width = label.len() as u32 * 7 + 10;
    let value_width = value.len() as u32 * 7 + 10;
    let total_width = label_width + value_width;

    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total}" height="20" role="img" aria-label="{label}: {value}">
  <title>{label}: {value}</title>
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="r"><rect width="{total}" height="20" rx="3" fill="#fff"/></clipPath>
  <g clip-path="url(#r)">
    <rect width="{lw}" height="20" fill="#555"/>
    <rect x="{lw}" width="{vw}" height="20" fill="{color}"/>
    <rect width="{total}" height="20" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="11">
    <text aria-hidden="true" x="{lx}" y="15" fill="#010101" fill-opacity=".3">{label}</text>
    <text x="{lx}" y="14">{label}</text>
    <text aria-hidden="true" x="{vx}" y="15" fill="#010101" fill-opacity=".3">{value}</text>
    <text x="{vx}" y="14">{value}</text>
  </g>
</svg>"##,
        total = total_width,
        lw = label_width,
        vw = value_width,
        color = color,
        lx = label_width / 2,
        vx = label_width + value_width / 2,
        label = label,
        value = value,
    );

    (
        [
            (header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300, stale-while-revalidate=3600"),
        ],
        svg,
    )
        .into_response()
}
