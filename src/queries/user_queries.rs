use sqlx::PgPool;

use crate::{
    error::Result,
    models::{User, UserAddress},
};

pub async fn create_user(
    pool: &PgPool,
    email: &str,
    name: &str,
    password_hash: &str,
) -> Result<User> {
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, name, password) VALUES ($1, $2, $3) RETURNING *",
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

pub async fn create_google_user(
    pool: &PgPool,
    email: &str,
    name: &str,
    google_id: &str,
) -> Result<User> {
    let user = sqlx::query_as::<_, User>(
        "INSERT INTO users (email, name, google_id) VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(email)
    .bind(name)
    .bind(google_id)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

pub async fn find_by_google_id(pool: &PgPool, google_id: &str) -> Result<Option<User>> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE google_id = $1")
        .bind(google_id)
        .fetch_optional(pool)
        .await?;

    Ok(user)
}

pub async fn add_user_address(
    pool: &PgPool,
    user_id: i32,
    payload: UserAddress,
) -> Result<UserAddress> {
    let address = sqlx::query_as::<_, UserAddress>(
        "INSERT INTO user_addresses (user_id, city, address, details) VALUES ($1, $2, $3, $4) RETURNING city, address, details"
    )
        .bind(user_id)
        .bind(payload.city)
        .bind(payload.address)
        .bind(payload.details)
        .fetch_one(pool)
        .await?;

    Ok(address)
}

pub async fn get_user_addresses(pool: &PgPool, user_id: i32) -> Result<Vec<UserAddress>> {
    let addresses = sqlx::query_as::<_, UserAddress>(
        "SELECT city, address, details FROM user_addresses WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(addresses)
}
