//! CLI helpers for API key management.

use crate::services::auth;
use agent_meter_db::Database;
use std::sync::Arc;
use uuid::Uuid;

pub fn generate_api_key_secret() -> String {
    format!("am_live_{}", Uuid::new_v4().simple())
}

pub async fn create_key(
    db: &Arc<dyn Database>,
    org_slug: &str,
    name: &str,
) -> anyhow::Result<String> {
    let org = db
        .find_org_by_slug(org_slug)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let secret = generate_api_key_secret();
    let prefix = auth::key_prefix(&secret)
        .ok_or_else(|| anyhow::anyhow!("generated key prefix too short"))?;
    let hash = auth::hash_key(&secret);
    db.create_api_key(org.id, name, prefix, &hash)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(secret)
}

pub async fn list_keys(db: &Arc<dyn Database>, org_slug: &str) -> anyhow::Result<()> {
    let org = db
        .find_org_by_slug(org_slug)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let keys = db
        .list_api_keys(org.id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if keys.is_empty() {
        println!("No API keys for org '{org_slug}'.");
        return Ok(());
    }
    for key in keys {
        println!(
            "{}  {}  prefix={}  created={}",
            key.id,
            key.name,
            key.key_prefix,
            key.created_at.format("%Y-%m-%d %H:%M UTC")
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_meter_db::SqliteDb;
    use std::sync::Arc;

    #[test]
    fn generated_secret_has_usable_prefix() {
        let secret = generate_api_key_secret();
        assert!(secret.starts_with("am_live_"));
        assert!(auth::key_prefix(&secret).is_some());
    }

    #[tokio::test]
    async fn create_key_roundtrip() {
        let path = std::env::temp_dir().join(format!("am-keys-test-{}.db", Uuid::new_v4()));
        let url = format!("sqlite://{}", path.display());
        let sqlite = SqliteDb::connect(&url).await.expect("connect");
        sqlite.migrate().await.expect("migrate");
        let db: Arc<dyn Database> = Arc::new(sqlite);

        let secret = create_key(&db, "personal", "test").await.expect("create");
        let prefix = auth::key_prefix(&secret).unwrap();
        let meta = db.find_key_by_prefix(prefix).await.expect("lookup");
        assert!(meta.is_some());
        assert_eq!(meta.unwrap().key_hash, auth::hash_key(&secret));

        let _ = std::fs::remove_file(path);
    }
}
