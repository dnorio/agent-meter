use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Db layer error: {0}")]
    Db(#[from] agent_meter_db::DbError),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Rate limit exceeded")]
    TooManyRequests,

    #[error("Ingest buffer full")]
    ServiceUnavailable,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(e) => {
                tracing::error!(error = %e, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".into(),
                )
            }
            AppError::Db(e) => match e {
                agent_meter_db::DbError::NotFound => (StatusCode::NOT_FOUND, "Not found".into()),
                agent_meter_db::DbError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
                agent_meter_db::DbError::Internal(msg) => {
                    tracing::error!(error = %msg, "db layer error");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Internal server error".into(),
                    )
                }
            },
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Internal(msg) => {
                tracing::error!(error = %msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".into(),
                )
            }
            AppError::TooManyRequests => {
                (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".into())
            }
            AppError::ServiceUnavailable => {
                (StatusCode::SERVICE_UNAVAILABLE, "Ingest buffer full".into())
            }
        };

        let body = json!({
            "error": message,
            "code": status.as_u16(),
        });

        if matches!(
            self,
            AppError::TooManyRequests | AppError::ServiceUnavailable
        ) {
            return (status, [(header::RETRY_AFTER, "5")], Json(body)).into_response();
        }

        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::StatusCode;

    async fn status_of(err: AppError) -> (StatusCode, serde_json::Value) {
        let resp = err.into_response();
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.expect("body");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        (status, json)
    }

    #[tokio::test]
    async fn into_response_maps_variants() {
        let cases: Vec<(AppError, StatusCode, &str)> = vec![
            (
                AppError::Db(agent_meter_db::DbError::NotFound),
                StatusCode::NOT_FOUND,
                "Not found",
            ),
            (
                AppError::Db(agent_meter_db::DbError::Conflict("dup".into())),
                StatusCode::CONFLICT,
                "dup",
            ),
            (
                AppError::Db(agent_meter_db::DbError::Internal("boom".into())),
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error",
            ),
            (
                AppError::Validation("bad".into()),
                StatusCode::BAD_REQUEST,
                "bad",
            ),
            (
                AppError::Unauthorized("nope".into()),
                StatusCode::UNAUTHORIZED,
                "nope",
            ),
            (
                AppError::NotFound("gone".into()),
                StatusCode::NOT_FOUND,
                "gone",
            ),
            (
                AppError::Internal("x".into()),
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error",
            ),
            (
                AppError::TooManyRequests,
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded",
            ),
            (
                AppError::ServiceUnavailable,
                StatusCode::SERVICE_UNAVAILABLE,
                "Ingest buffer full",
            ),
        ];
        for (err, want_status, want_msg) in cases {
            let (status, body) = status_of(err).await;
            assert_eq!(status, want_status);
            assert_eq!(body["error"], want_msg);
            assert_eq!(body["code"], want_status.as_u16());
        }
    }

    #[tokio::test]
    async fn retry_after_on_throttle_errors() {
        for err in [AppError::TooManyRequests, AppError::ServiceUnavailable] {
            let resp = err.into_response();
            assert_eq!(resp.headers().get(header::RETRY_AFTER).unwrap(), "5");
        }
    }
}
