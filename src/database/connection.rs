use crate::{config::DatabaseConfig, error::Result};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::str::FromStr;
use std::time::Duration;

pub async fn create_pool(config: &DatabaseConfig) -> Result<PgPool> {
    let opts = PgConnectOptions::from_str(&config.url)?.statement_cache_capacity(0);

    let pool = PgPoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(0)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(60 * 2)))
        .max_lifetime(Some(Duration::from_secs(60 * 5)))
        .test_before_acquire(false)
        .connect_with(opts)
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
