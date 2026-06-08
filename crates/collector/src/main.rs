use clap::{Parser, Subcommand};
use std::sync::Arc;

use agent_meter_collector::{config, db, run};
use agent_meter_db::{Database, SqliteDb};

#[derive(Parser)]
#[command(name = "agent-meter", version, about = "AI agent observability & FinOps collector")]
struct Cli {
    /// Path to config file (TOML). Env vars override file values.
    #[arg(short, long, env = "AGENT_METER_CONFIG")]
    config: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the collector server (default)
    Serve,
    /// Run database migrations
    Migrate,
    /// Print version and build info
    Version,
    /// Validate config and test DB connection
    Check,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load config from TOML file (if provided) then overlay env vars
    let cfg = if let Some(ref path) = cli.config {
        config::Config::from_file_and_env(path)?
    } else {
        config::Config::from_env()
    };

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => {
            if cfg.database_url.starts_with("sqlite:") {
                let sqlite_db = SqliteDb::connect(&cfg.database_url).await?;
                sqlite_db.migrate().await.map_err(|e| anyhow::anyhow!("{e}"))?;
                let _db: Arc<dyn Database> = Arc::new(sqlite_db);
                // For SQLite mode, we don't have a PgPool. Create a dummy connection
                // that will fail if services try to use it directly.
                // TODO: Remove pool from AppState once all services use db trait.
                eprintln!("⚠ SQLite mode: services using pool directly will not work until migrated to db trait");
                anyhow::bail!("SQLite serve mode requires services migrated to db trait (WIP — use postgres for now)");
            } else {
                let pool = db::connect(&cfg.database_url).await?;
                run(cfg, pool).await
            }
        }
        Command::Migrate => {
            if cfg.database_url.starts_with("sqlite:") {
                let sqlite_db = SqliteDb::connect(&cfg.database_url).await?;
                sqlite_db.migrate().await.map_err(|e| anyhow::anyhow!("{e}"))?;
            } else {
                let pool = db::connect(&cfg.database_url).await?;
                sqlx::migrate!("../../migrations")
                    .run(&pool)
                    .await?;
            }
            println!("✓ Migrations applied successfully");
            Ok(())
        }
        Command::Version => {
            println!(
                "agent-meter {} ({})",
                env!("CARGO_PKG_VERSION"),
                if cfg!(debug_assertions) { "debug" } else { "release" }
            );
            Ok(())
        }
        Command::Check => {
            println!("Config: {:?}", cfg.host);
            println!("Database: {}", mask_url(&cfg.database_url));
            if cfg.database_url.starts_with("sqlite:") {
                let sqlite_db = SqliteDb::connect(&cfg.database_url).await?;
                sqlite_db.health_check().await.map_err(|e| anyhow::anyhow!("{e}"))?;
                println!("✓ SQLite connection OK");
            } else {
                let pool = db::connect(&cfg.database_url).await?;
                let row: (i32,) = sqlx::query_as("SELECT 1")
                    .fetch_one(&pool)
                    .await?;
                println!("✓ Database connection OK (test query returned {})", row.0);
            }
            Ok(())
        }
    }
}

fn mask_url(url: &str) -> String {
    if let Some(at) = url.find('@') {
        if let Some(colon) = url[..at].rfind(':') {
            return format!("{}:****@{}", &url[..colon], &url[at + 1..]);
        }
    }
    url.to_string()
}
