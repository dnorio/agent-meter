use axum::{
    extract::{ConnectInfo, Request},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::net::{IpAddr, SocketAddr};

pub async fn localhost_only(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    if is_loopback(addr.ip()) {
        next.run(request).await
    } else {
        (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": "admin endpoints are localhost-only",
                "code": 403
            })),
        )
            .into_response()
    }
}

fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.octets()[0] == 127,
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_detection() {
        assert!(is_loopback("127.0.0.1".parse().unwrap()));
        assert!(is_loopback("127.1.2.3".parse().unwrap()));
        assert!(is_loopback("::1".parse().unwrap()));
        assert!(!is_loopback("10.0.0.1".parse().unwrap()));
        assert!(!is_loopback("192.168.1.1".parse().unwrap()));
    }
}
