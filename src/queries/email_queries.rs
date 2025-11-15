use chrono::{Duration, Utc};
use sqlx::PgPool;

use crate::{error::Result, models::VerificationCode};

const CODE_EXPIRY_MINUTES: i64 = 5;

pub async fn create_verification_code(
    pool: &PgPool,
    email: &str,
    code: i32,
) -> Result<VerificationCode> {
    let expires_at = Utc::now() + Duration::minutes(CODE_EXPIRY_MINUTES);

    let verification_code = sqlx::query_as::<_, VerificationCode>(
        "INSERT INTO email_verification_codes (email, code, expires_at)
         VALUES ($1, $2, $3)
         RETURNING *",
    )
    .bind(email)
    .bind(code)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;

    Ok(verification_code)
}

pub async fn find_valid_code(pool: &PgPool, email: &str, code: i32) -> Result<Option<VerificationCode>> {
    let verification_code = sqlx::query_as::<_, VerificationCode>(
        "SELECT * FROM email_verification_codes
         WHERE email = $1 AND code = $2 AND expires_at > NOW()
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(email)
    .bind(code)
    .fetch_optional(pool)
    .await?;

    Ok(verification_code)
}

pub async fn delete_code(pool: &PgPool, id: i32) -> Result<()> {
    sqlx::query("DELETE FROM email_verification_codes WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn delete_codes_for_email(pool: &PgPool, email: &str) -> Result<()> {
    sqlx::query("DELETE FROM email_verification_codes WHERE email = $1")
        .bind(email)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn cleanup_expired_codes(pool: &PgPool) -> Result<()> {
    sqlx::query("DELETE FROM email_verification_codes WHERE expires_at < NOW()")
        .execute(pool)
        .await?;

    Ok(())
}
