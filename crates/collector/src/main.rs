use clap::{Parser, Subcommand};
use std::sync::Arc;

use agent_meter_collector::{config, db, keys, run};
use agent_meter_db::{Database, PostgresDb, SqliteDb};

/// Build the backend-agnostic database handle from the configured URL.
/// SQLite is the default (single-file, zero-config standalone); Postgres is
/// used when `DATABASE_URL` starts with `postgres:`.
async fn connect_db(database_url: &str) -> anyhow::Result<Arc<dyn Database>> {
    if database_url.starts_with("sqlite:") {
        let sqlite_db = SqliteDb::connect(database_url).await?;
        sqlite_db
            .migrate()
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(Arc::new(sqlite_db))
    } else {
        let pool = db::connect(database_url).await?;
        let pg = PostgresDb::from_pool(pool);
        pg.migrate().await.map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(Arc::new(pg))
    }
}

#[derive(Parser)]
#[command(
    name = "agent-meter",
    version,
    about = "AI agent observability & FinOps collector"
)]
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
    /// Seed synthetic data and start the server (showcase mode)
    Demo {
        /// Number of synthetic conversations to generate
        #[arg(long, default_value_t = 6)]
        conversations: usize,
        /// Tool-call events per conversation
        #[arg(long, default_value_t = 10)]
        events: usize,
        /// Seed again even if the database already has data
        #[arg(long)]
        force: bool,
    },
    /// Run database migrations
    Migrate,
    /// Print version and build info
    Version,
    /// Validate config and test DB connection
    Check,
    /// Manage API keys (for SDK ingest auth)
    Keys {
        #[command(subcommand)]
        action: KeysAction,
    },
}

#[derive(Subcommand)]
enum KeysAction {
    /// Create a new API key (secret shown once)
    Create {
        /// Key label
        #[arg(long, default_value = "default")]
        name: String,
        /// Organization slug
        #[arg(long, default_value = "personal")]
        org: String,
    },
    /// List API keys for an organization (prefixes only)
    List {
        /// Organization slug
        #[arg(long, default_value = "personal")]
        org: String,
    },
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
            let db = connect_db(&cfg.database_url).await?;
            run(cfg, db).await
        }
        Command::Demo {
            conversations,
            events,
            force,
        } => {
            let db = connect_db(&cfg.database_url).await?;
            if force || !agent_meter_collector::demo::has_data(&db).await {
                let n = agent_meter_collector::demo::seed(&db, conversations, events).await?;
                println!("✓ seeded {n} synthetic events across {conversations} conversations");
            } else {
                println!("• database already has data — starting without re-seeding (use --force to reseed)");
            }
            run(cfg, db).await
        }
        Command::Migrate => {
            let _db = connect_db(&cfg.database_url).await?;
            println!("✓ Migrations applied successfully");
            Ok(())
        }
        Command::Version => {
            println!(
                "agent-meter {} ({})",
                env!("CARGO_PKG_VERSION"),
                if cfg!(debug_assertions) {
                    "debug"
                } else {
                    "release"
                }
            );
            Ok(())
        }
        Command::Check => {
            println!("Config: {:?}", cfg.host);
            println!("Database: {}", mask_url(&cfg.database_url));
            if cfg.database_url.starts_with("sqlite:") {
                let sqlite_db = SqliteDb::connect(&cfg.database_url).await?;
                sqlite_db
                    .health_check()
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                println!("✓ SQLite connection OK");
            } else {
                let pool = db::connect(&cfg.database_url).await?;
                let row: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await?;
                println!("✓ Database connection OK (test query returned {})", row.0);
            }
            Ok(())
        }
        Command::Keys { action } => {
            let db = connect_db(&cfg.database_url).await?;
            match action {
                KeysAction::Create { name, org } => {
                    let secret = keys::create_key(&db, &org, &name).await?;
                    println!("✓ API key created for org '{org}' (name: {name})");
                    println!();
                    println!("  {secret}");
                    println!();
                    println!("Store this secret now — it won't be shown again.");
                    println!("Use: export AGENT_METER_API_KEY='{secret}'");
                    Ok(())
                }
                KeysAction::List { org } => keys::list_keys(&db, &org).await,
            }
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
