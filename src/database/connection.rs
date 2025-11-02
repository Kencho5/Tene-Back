use crate::{config::DatabaseConfig, error::Result};
use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn create_pool(config: &DatabaseConfig) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(&config.url)
        .await?;

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!(
        "Database connection established with {} max connections",
        config.max_connections
    );

    Ok(pool)
}

pub async fn check_health(pool: &PgPool) -> Result<()> {
    sqlx::query("SELECT 1").fetch_one(pool).await?;
    Ok(())
}
