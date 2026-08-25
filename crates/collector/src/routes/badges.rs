//! Shields-style SVG badges for README embeds (`/badge/cost.svg`, `/badge/events.svg`).

use axum::{
    extract::{Path, Query, State},
    http::header,
    response::Response,
    routing::get,
    Router,
};
use badgelib::{Badge, Color, Style};
use chrono::{Duration, Utc};

use agent_meter_db::params::CostQuery;

use crate::app::AppState;
use crate::errors::AppError;

#[derive(serde::Deserialize)]
struct BadgeParams {
    style: Option<Style>,
}

impl BadgeParams {
    fn style(self) -> Style {
        self.style.unwrap_or(Style::FlatSquare)
    }
}

async fn cost_badge(
    State(state): State<AppState>,
    Query(params): Query<BadgeParams>,
) -> Result<Response, AppError> {
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
    Ok(svg_response("AI cost", &message, "#007ec6", params.style()))
}

async fn events_badge(
    State(state): State<AppState>,
    Path(kind): Path<String>,
    Query(params): Query<BadgeParams>,
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
    Ok(svg_response("events", &message, "#4c1", params.style()))
}

fn svg_response(label: &str, message: &str, color: &str, style: Style) -> Response {
    let body = Badge::new()
        .label(label)
        .label_color(Color::Hex("555".into()))
        .value(message)
        .value_color(Color::Hex(color.trim_start_matches('#').into()))
        .style(style)
        .to_svg();
    Response::builder()
        .header(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")
        .header(header::CACHE_CONTROL, "public, max-age=300")
        .body(body.into())
        .expect("badge response")
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/badge/cost.svg", get(cost_badge))
        .route("/badge/{kind}", get(events_badge))
}
