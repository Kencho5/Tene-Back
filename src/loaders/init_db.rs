use crate::prelude::*;

pub async fn init_db() -> PgPool {
    let db_url = env::var("DB_URL").expect("DB url not set");
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&db_url)
        .await
        .expect("Failed to connect to the database");

    pool
}
