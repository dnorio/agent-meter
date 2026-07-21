//! Per-IP rate limiter for ingest endpoints.
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

#[derive(Debug)]
pub struct RateLimitExceeded;

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
    pub fn check(&self, ip: &str) -> Result<u32, RateLimitExceeded> {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();

        let entry = buckets.entry(ip.to_string()).or_insert((0, now));

        // Reset window if expired
        if now.duration_since(entry.1).as_secs() >= self.window_secs {
            *entry = (0, now);
        }

        if entry.0 >= self.max_requests {
            return Err(RateLimitExceeded);
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
pub async fn rate_limit_ingest(req: Request<Body>, next: Next) -> Response {
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
            Err(RateLimitExceeded) => (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", "60")],
                "rate limit exceeded",
            )
                .into_response(),
        }
    } else {
        // No limiter configured — pass through
        next.run(req).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use std::sync::Arc;
    use std::time::Duration;
    use tower::ServiceExt;

    #[test]
    fn allows_requests_below_limit_and_blocks_at_capacity() {
        let limiter = RateLimiter::new(3, 60);

        assert_eq!(limiter.check("10.0.0.1").expect("first"), 2);
        assert_eq!(limiter.check("10.0.0.1").expect("second"), 1);
        assert_eq!(limiter.check("10.0.0.1").expect("third"), 0);
        assert!(limiter.check("10.0.0.1").is_err());
    }

    #[test]
    fn tracks_ips_independently() {
        let limiter = RateLimiter::new(1, 60);

        assert_eq!(limiter.check("10.0.0.1").expect("ip1"), 0);
        assert!(limiter.check("10.0.0.1").is_err());
        assert_eq!(limiter.check("10.0.0.2").expect("ip2"), 0);
    }

    #[test]
    fn resets_window_after_expiry() {
        let limiter = RateLimiter::new(1, 1);

        assert_eq!(limiter.check("10.0.0.3").expect("first"), 0);
        assert!(limiter.check("10.0.0.3").is_err());

        std::thread::sleep(Duration::from_secs(2));

        assert_eq!(limiter.check("10.0.0.3").expect("after reset"), 0);
    }

    #[tokio::test]
    async fn middleware_blocks_ingest_path_when_limit_exceeded() {
        let limiter = Arc::new(RateLimiter::new(1, 60));
        let app = Router::new()
            .route("/v1/traces", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(rate_limit_ingest))
            .layer(axum::Extension(limiter));

        let req = Request::builder()
            .uri("/v1/traces")
            .header("x-forwarded-for", "203.0.113.10")
            .body(Body::empty())
            .unwrap();

        let first = app.clone().oneshot(req).await.unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(first.headers().get("x-ratelimit-remaining").unwrap(), "0");

        let req = Request::builder()
            .uri("/v1/traces")
            .header("x-forwarded-for", "203.0.113.10")
            .body(Body::empty())
            .unwrap();

        let second = app.oneshot(req).await.unwrap();
        assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn middleware_passes_through_non_ingest_paths() {
        let limiter = Arc::new(RateLimiter::new(1, 60));
        let app = Router::new()
            .route("/health", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(rate_limit_ingest))
            .layer(axum::Extension(limiter));

        for _ in 0..3 {
            let req = Request::builder()
                .uri("/health")
                .header("x-forwarded-for", "203.0.113.11")
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }
    }
}
