use clap::{Parser, Subcommand};

use agent_meter_collector::{config, db, run};

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
            let pool = db::connect(&cfg.database_url).await?;
            run(cfg, pool).await
        }
        Command::Migrate => {
            let pool = db::connect(&cfg.database_url).await?;
            sqlx::migrate!("../../migrations")
                .run(&pool)
                .await?;
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
            let pool = db::connect(&cfg.database_url).await?;
            let row: (i32,) = sqlx::query_as("SELECT 1")
                .fetch_one(&pool)
                .await?;
            println!("✓ Database connection OK (test query returned {})", row.0);
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
