use agent_meter_collector::{config, db, run};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::from_env();
    let pool = db::connect(&config.database_url).await?;
    run(config, pool).await
}
