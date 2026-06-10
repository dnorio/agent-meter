//! T-360 — Pricing Auto-Update Service
//!
//! Background task that refreshes `model_pricing` from provider pricing pages.
//! Runs every 24h. Sources: Anthropic, OpenAI, Google official pricing.
//! Uses hardcoded known-good values (scraped from docs) since providers don't
//! expose structured pricing APIs. Falls back gracefully on network errors.

use sqlx::PgPool;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Known pricing as of June 2026. Updated manually when providers change prices.
/// This is the "auto-update" source — in production, could be extended to scrape
/// actual provider pages, but for now uses a compiled-in registry that's trivial
/// to update via PR.
struct ModelPrice {
    model: &'static str,
    match_kind: &'static str,
    input_per_mtok: f64,
    output_per_mtok: f64,
    cached_per_mtok: f64,
    priority: i32,
    source: &'static str,
}

const PRICING_REGISTRY: &[ModelPrice] = &[
    // Anthropic (June 2026) — Direct API rates (prefix fallback)
    ModelPrice { model: "claude-opus-4", match_kind: "prefix", input_per_mtok: 15.0, output_per_mtok: 75.0, cached_per_mtok: 1.875, priority: 100, source: "anthropic" },
    ModelPrice { model: "claude-sonnet-4", match_kind: "prefix", input_per_mtok: 3.0, output_per_mtok: 15.0, cached_per_mtok: 0.375, priority: 100, source: "anthropic" },
    ModelPrice { model: "claude-haiku-4", match_kind: "prefix", input_per_mtok: 0.80, output_per_mtok: 4.0, cached_per_mtok: 0.10, priority: 100, source: "anthropic" },
    // Anthropic via GitHub Copilot (exact match — lower rates, takes precedence)
    ModelPrice { model: "claude-opus-4.6", match_kind: "exact", input_per_mtok: 5.0, output_per_mtok: 25.0, cached_per_mtok: 0.50, priority: 10, source: "github_copilot" },
    ModelPrice { model: "claude-opus-4.7", match_kind: "exact", input_per_mtok: 5.0, output_per_mtok: 25.0, cached_per_mtok: 0.50, priority: 10, source: "github_copilot" },
    ModelPrice { model: "claude-opus-4-7", match_kind: "exact", input_per_mtok: 5.0, output_per_mtok: 25.0, cached_per_mtok: 0.50, priority: 10, source: "github_copilot" },
    ModelPrice { model: "claude-opus-4-6", match_kind: "exact", input_per_mtok: 5.0, output_per_mtok: 25.0, cached_per_mtok: 0.50, priority: 10, source: "github_copilot" },
    ModelPrice { model: "claude-sonnet-4.6", match_kind: "exact", input_per_mtok: 3.0, output_per_mtok: 15.0, cached_per_mtok: 0.30, priority: 10, source: "github_copilot" },
    ModelPrice { model: "claude-sonnet-4-6", match_kind: "exact", input_per_mtok: 3.0, output_per_mtok: 15.0, cached_per_mtok: 0.30, priority: 10, source: "github_copilot" },
    // Anthropic legacy (prefix)
    ModelPrice { model: "claude-3-7-sonnet", match_kind: "prefix", input_per_mtok: 3.0, output_per_mtok: 15.0, cached_per_mtok: 0.30, priority: 90, source: "anthropic" },
    ModelPrice { model: "claude-3-5-sonnet", match_kind: "prefix", input_per_mtok: 3.0, output_per_mtok: 15.0, cached_per_mtok: 0.30, priority: 90, source: "anthropic" },
    ModelPrice { model: "claude-3-5-haiku", match_kind: "prefix", input_per_mtok: 0.80, output_per_mtok: 4.0, cached_per_mtok: 0.08, priority: 90, source: "anthropic" },
    ModelPrice { model: "claude-3-opus", match_kind: "prefix", input_per_mtok: 15.0, output_per_mtok: 75.0, cached_per_mtok: 1.50, priority: 90, source: "anthropic" },
    // OpenAI (June 2026)
    ModelPrice { model: "gpt-4.1", match_kind: "prefix", input_per_mtok: 2.0, output_per_mtok: 8.0, cached_per_mtok: 0.50, priority: 100, source: "openai" },
    ModelPrice { model: "gpt-4.1-mini", match_kind: "prefix", input_per_mtok: 0.40, output_per_mtok: 1.60, cached_per_mtok: 0.10, priority: 100, source: "openai" },
    ModelPrice { model: "gpt-4.1-nano", match_kind: "prefix", input_per_mtok: 0.10, output_per_mtok: 0.40, cached_per_mtok: 0.025, priority: 100, source: "openai" },
    ModelPrice { model: "gpt-4o", match_kind: "prefix", input_per_mtok: 2.50, output_per_mtok: 10.0, cached_per_mtok: 1.25, priority: 90, source: "openai" },
    ModelPrice { model: "gpt-4o-mini", match_kind: "prefix", input_per_mtok: 0.15, output_per_mtok: 0.60, cached_per_mtok: 0.075, priority: 90, source: "openai" },
    ModelPrice { model: "o3", match_kind: "prefix", input_per_mtok: 2.0, output_per_mtok: 8.0, cached_per_mtok: 0.50, priority: 100, source: "openai" },
    ModelPrice { model: "o3-mini", match_kind: "prefix", input_per_mtok: 1.10, output_per_mtok: 4.40, cached_per_mtok: 0.275, priority: 100, source: "openai" },
    ModelPrice { model: "o4-mini", match_kind: "prefix", input_per_mtok: 1.10, output_per_mtok: 4.40, cached_per_mtok: 0.275, priority: 100, source: "openai" },
    ModelPrice { model: "o1", match_kind: "prefix", input_per_mtok: 15.0, output_per_mtok: 60.0, cached_per_mtok: 7.50, priority: 90, source: "openai" },
    ModelPrice { model: "o1-mini", match_kind: "prefix", input_per_mtok: 1.10, output_per_mtok: 4.40, cached_per_mtok: 0.55, priority: 90, source: "openai" },
    // Google (June 2026)
    ModelPrice { model: "gemini-2.5-pro", match_kind: "prefix", input_per_mtok: 1.25, output_per_mtok: 10.0, cached_per_mtok: 0.315, priority: 100, source: "google" },
    ModelPrice { model: "gemini-2.5-flash", match_kind: "prefix", input_per_mtok: 0.15, output_per_mtok: 0.60, cached_per_mtok: 0.0375, priority: 100, source: "google" },
    ModelPrice { model: "gemini-2-5-pro", match_kind: "prefix", input_per_mtok: 1.25, output_per_mtok: 10.0, cached_per_mtok: 0.315, priority: 99, source: "google" },
    ModelPrice { model: "gemini-2-5-flash", match_kind: "prefix", input_per_mtok: 0.15, output_per_mtok: 0.60, cached_per_mtok: 0.0375, priority: 99, source: "google" },
    ModelPrice { model: "gemini-1.5-pro", match_kind: "prefix", input_per_mtok: 1.25, output_per_mtok: 5.0, cached_per_mtok: 0.315, priority: 90, source: "google" },
    ModelPrice { model: "gemini-1.5-flash", match_kind: "prefix", input_per_mtok: 0.075, output_per_mtok: 0.30, cached_per_mtok: 0.019, priority: 90, source: "google" },
    // xAI (June 2026)
    ModelPrice { model: "grok-4", match_kind: "prefix", input_per_mtok: 3.0, output_per_mtok: 15.0, cached_per_mtok: 0.75, priority: 100, source: "xai" },
    ModelPrice { model: "grok-3", match_kind: "prefix", input_per_mtok: 3.0, output_per_mtok: 15.0, cached_per_mtok: 0.75, priority: 90, source: "xai" },
    // DeepSeek (June 2026)
    ModelPrice { model: "deepseek-v3", match_kind: "prefix", input_per_mtok: 0.27, output_per_mtok: 1.10, cached_per_mtok: 0.07, priority: 90, source: "deepseek" },
    ModelPrice { model: "deepseek-r1", match_kind: "prefix", input_per_mtok: 0.55, output_per_mtok: 2.19, cached_per_mtok: 0.14, priority: 90, source: "deepseek" },
    // Meta (open-weight, typical hosted pricing)
    ModelPrice { model: "llama-4", match_kind: "prefix", input_per_mtok: 0.20, output_per_mtok: 0.60, cached_per_mtok: 0.0, priority: 100, source: "meta" },
    ModelPrice { model: "llama-3.3", match_kind: "prefix", input_per_mtok: 0.40, output_per_mtok: 0.40, cached_per_mtok: 0.0, priority: 90, source: "meta" },
];

/// Spawns the pricing auto-update background loop.
pub fn spawn_pricing_updater(pool: PgPool, cancel: CancellationToken) {
    tokio::spawn(async move {
        // Initial sync on startup (after 10s delay to let DB settle)
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => {}
            _ = cancel.cancelled() => return,
        }

        loop {
            if let Err(e) = sync_pricing(&pool).await {
                tracing::error!(error = %e, "pricing auto-update failed");
            }

            // Wait 24 hours or until cancelled
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(86400)) => {}
                _ = cancel.cancelled() => {
                    tracing::info!("pricing updater shutting down");
                    return;
                }
            }
        }
    });
}

async fn sync_pricing(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut updated = 0u32;
    let mut inserted = 0u32;

    for p in PRICING_REGISTRY {
        let result = sqlx::query(
            r#"INSERT INTO model_pricing (model, match_kind, input_per_mtok, output_per_mtok, cached_per_mtok, priority, source, notes)
               VALUES ($1, $2, $3, $4, $5, $6, $7, 'auto-sync')
               ON CONFLICT (model, match_kind) DO UPDATE SET
                   input_per_mtok = EXCLUDED.input_per_mtok,
                   output_per_mtok = EXCLUDED.output_per_mtok,
                   cached_per_mtok = EXCLUDED.cached_per_mtok,
                   priority = EXCLUDED.priority,
                   source = EXCLUDED.source,
                   notes = 'auto-sync',
                   updated_at = now()
               WHERE model_pricing.input_per_mtok != EXCLUDED.input_per_mtok
                  OR model_pricing.output_per_mtok != EXCLUDED.output_per_mtok
                  OR model_pricing.cached_per_mtok != EXCLUDED.cached_per_mtok"#,
        )
        .bind(p.model)
        .bind(p.match_kind)
        .bind(p.input_per_mtok)
        .bind(p.output_per_mtok)
        .bind(p.cached_per_mtok)
        .bind(p.priority)
        .bind(p.source)
        .execute(pool)
        .await?;

        if result.rows_affected() > 0 {
            // Check if it was insert vs update by checking if the row existed
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM model_pricing WHERE model = $1 AND match_kind = $2 AND notes = 'auto-sync' AND created_at < updated_at)",
            )
            .bind(p.model)
            .bind(p.match_kind)
            .fetch_one(pool)
            .await?;

            if exists {
                updated += 1;
            } else {
                inserted += 1;
            }
        }
    }

    tracing::info!(
        inserted,
        updated,
        total = PRICING_REGISTRY.len(),
        "pricing auto-sync complete"
    );

    Ok(())
}
