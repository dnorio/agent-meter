//! T-322 — Per-IP rate limiter for ingest endpoints.
//!
//! Uses a sliding window counter per client IP. Lightweight, in-memory,
//! zero external dependencies. Designed for the OTLP /v1/traces path
//! to prevent abuse without penalizing normal SDK telemetry.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Rate limiter state — shared across all requests.
#[derive(Debug)]
pub struct RateLimiter {
    /// Max requests per window per IP.
    max_requests: u32,
    /// Window duration in seconds.
    window_secs: u64,
    /// IP → (count, window_start)
    buckets: Mutex<HashMap<String, (u32, Instant)>>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Check if the IP is within rate limit. Returns remaining requests.
    pub fn check(&self, ip: &str) -> Result<u32, ()> {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();

        let entry = buckets.entry(ip.to_string()).or_insert((0, now));

        // Reset window if expired
        if now.duration_since(entry.1).as_secs() >= self.window_secs {
            *entry = (0, now);
        }

        if entry.0 >= self.max_requests {
            return Err(());
        }

        entry.0 += 1;
        Ok(self.max_requests - entry.0)
    }

    /// Periodic cleanup of expired entries (call from background task).
    pub fn cleanup(&self) {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        buckets.retain(|_, (_, start)| now.duration_since(*start).as_secs() < self.window_secs * 2);
    }
}

/// Axum middleware layer for rate limiting ingest endpoints.
pub async fn rate_limit_ingest(
    req: Request<Body>,
    next: Next,
) -> Response {
    // Only rate-limit the OTLP ingest path
    let path = req.uri().path();
    if !path.starts_with("/v1/traces") && !path.starts_with("/events/tool-call") {
        return next.run(req).await;
    }

    // Extract client IP from X-Forwarded-For or X-Real-IP
    let ip = req
        .headers()
        .get("x-forwarded-for")
        .or_else(|| req.headers().get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    // Get rate limiter from extensions
    let limiter = req.extensions().get::<std::sync::Arc<RateLimiter>>();
    if let Some(limiter) = limiter {
        match limiter.check(&ip) {
            Ok(remaining) => {
                let mut resp = next.run(req).await;
                resp.headers_mut().insert(
                    "x-ratelimit-remaining",
                    remaining.to_string().parse().unwrap(),
                );
                resp
            }
            Err(()) => {
                (
                    StatusCode::TOO_MANY_REQUESTS,
                    [("retry-after", "60")],
                    "rate limit exceeded",
                )
                    .into_response()
            }
        }
    } else {
        // No limiter configured — pass through
        next.run(req).await
    }
}
