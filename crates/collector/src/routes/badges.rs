//! Shields-style SVG badges for README embeds (`/badge/cost.svg`, `/badge/events.svg`).

use axum::{
    extract::{Path, State},
    http::header,
    response::Response,
    routing::get,
    Router,
};
use chrono::{Duration, Utc};

use agent_meter_db::params::CostQuery;

use crate::app::AppState;
use crate::errors::AppError;

async fn cost_badge(State(state): State<AppState>) -> Result<Response, AppError> {
    let to = Utc::now();
    let from = to - Duration::days(30);
    let summary = state
        .db
        .cost_summary(&CostQuery {
            from,
            to,
            model: None,
        })
        .await?;
    let usd = summary.kpis.total_usd;
    let message = if usd >= 100.0 {
        format!("${usd:.0}")
    } else if usd >= 1.0 {
        format!("${usd:.2}")
    } else {
        format!("${usd:.4}")
    };
    Ok(svg_response("AI cost", &message, "#007ec6"))
}

async fn events_badge(
    State(state): State<AppState>,
    Path(kind): Path<String>,
) -> Result<Response, AppError> {
    if kind != "events.svg" {
        return Err(AppError::NotFound(format!("unknown badge: {kind}")));
    }
    let to = Utc::now();
    let from = to - Duration::days(30);
    let summary = state
        .db
        .cost_summary(&CostQuery {
            from,
            to,
            model: None,
        })
        .await?;
    let count = summary.kpis.total_events;
    let message = format!("{count}");
    Ok(svg_response("events", &message, "#4c1"))
}

fn svg_response(label: &str, message: &str, color: &str) -> Response {
    let label = xml_escape(label);
    let message = xml_escape(message);
    let lw = (label.len() as u32 * 7).max(40) + 10;
    let rw = (message.len() as u32 * 7).max(24) + 10;
    let w = lw + rw;
    let lx = lw / 2;
    let mx = lw + rw / 2;
    let body = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="20" role="img" aria-label="{label}: {message}">
<title>{label}: {message}</title>
<rect width="{lw}" height="20" fill="#555"/>
<rect x="{lw}" width="{rw}" height="20" fill="{color}"/>
<text x="{lx}" y="14" fill="#fff" font-size="11" font-family="Verdana,Geneva,sans-serif" text-anchor="middle">{label}</text>
<text x="{mx}" y="14" fill="#fff" font-size="11" font-family="Verdana,Geneva,sans-serif" text-anchor="middle">{message}</text>
</svg>"##
    );
    Response::builder()
        .header(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=300")
        .body(body.into())
        .expect("badge response")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/badge/cost.svg", get(cost_badge))
        .route("/badge/{kind}", get(events_badge))
}
