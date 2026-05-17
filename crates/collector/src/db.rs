use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await?;

    tracing::info!("connected to PostgreSQL");
    Ok(pool)
}
