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
    use axum::body::Body;
    use axum::routing::get;
    use axum::Router;
    use std::net::{Ipv4Addr, SocketAddr};
    use tower::Service;
    use tower::ServiceExt;

    #[test]
    fn loopback_detection() {
        assert!(is_loopback("127.0.0.1".parse().unwrap()));
        assert!(is_loopback("127.1.2.3".parse().unwrap()));
        assert!(is_loopback("::1".parse().unwrap()));
        assert!(!is_loopback("10.0.0.1".parse().unwrap()));
        assert!(!is_loopback("192.168.1.1".parse().unwrap()));
    }

    #[tokio::test]
    async fn middleware_allows_loopback_connect_info() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(localhost_only));

        let mut svc = app.into_service();
        let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from((Ipv4Addr::LOCALHOST, 8081))));

        let resp = svc.ready().await.unwrap().call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_blocks_non_loopback_connect_info() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(axum::middleware::from_fn(localhost_only));

        let mut svc = app.into_service();
        let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
        req.extensions_mut().insert(ConnectInfo(SocketAddr::from((
            Ipv4Addr::new(10, 0, 0, 1),
            8081,
        ))));

        let resp = svc.ready().await.unwrap().call(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }
}
