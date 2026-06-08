//! T-322 — Row-Level Security extractor: extracts org_id from session cookie.
//!
//! Handlers that need tenant isolation can add `OrgContext` as an extractor:
//! ```ignore
//! async fn my_handler(org: OrgContext, State(state): State<AppState>) -> ... {
//!     if let Some(oid) = org.org_id { /* filter by org */ }
//! }
//! ```
//!
//! If no session is present, org_id is None (public/anonymous access).

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use uuid::Uuid;

use crate::app::AppState;
use crate::services::auth_service;

/// Context extracted per-request with the authenticated org_id (if any).
#[derive(Clone, Debug)]
pub struct OrgContext {
    pub org_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
}

#[async_trait]
impl FromRequestParts<AppState> for OrgContext {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let cookie_header = parts
            .headers
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let token = cookie_header
            .split(';')
            .filter_map(|c| c.trim().strip_prefix("am_session="))
            .next();

        let Some(token) = token else {
            return Ok(OrgContext {
                org_id: None,
                user_id: None,
            });
        };

        match auth_service::lookup_session(&state.pool, token).await {
            Ok(Some(user)) => Ok(OrgContext {
                org_id: user.org_id,
                user_id: Some(user.user_id),
            }),
            _ => Ok(OrgContext {
                org_id: None,
                user_id: None,
            }),
        }
    }
}

