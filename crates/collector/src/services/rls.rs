//! T-322 — Row-Level Security (RLS) helpers.
//!
//! Provides `with_rls()` which acquires a connection from the pool, sets
//! `app.current_org_id` for the lifetime of the transaction, and returns
//! a transaction handle that handlers can use for queries.
//!
//! ## Usage
//!
//! ```ignore
//! use crate::services::rls;
//!
//! let mut tx = rls::begin(&pool, org_id).await?;
//! let rows = sqlx::query("SELECT * FROM agent_tool_calls")
//!     .fetch_all(&mut *tx)
//!     .await?;
//! tx.commit().await?;
//! ```
//!
//! The policy `tenant_isolation` on each table will automatically filter
//! rows by the org_id set via `SET LOCAL`.

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::errors::AppError;

/// Begin a transaction with RLS context set to the given org_id.
/// If org_id is None, no SET LOCAL is issued (backward-compat: superuser
/// bypasses RLS or all data is accessible).
pub async fn begin(
    pool: &PgPool,
    org_id: Option<Uuid>,
) -> Result<Transaction<'static, Postgres>, AppError> {
    let mut tx = pool.begin().await?;

    if let Some(oid) = org_id {
        // SET LOCAL only affects the current transaction.
        // org_id is a Uuid (validated hex+hyphens only) — safe from injection.
        sqlx::query("SELECT set_config('app.current_org_id', $1, true)")
            .bind(oid.to_string())
            .execute(&mut *tx)
            .await?;
    }

    Ok(tx)
}
