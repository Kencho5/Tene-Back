use sqlx::PgPool;

use crate::{error::Result, models::User};

pub async fn create_user(pool: &PgPool, email: &str, name: &str, password_hash: &str) -> Result<User> {
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, name, password) VALUES ($1, $2, $3) RETURNING *"
    )
    .bind(email)
    .bind(name)
    .bind(password_hash)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await?;

    Ok(user)
}
