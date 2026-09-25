use chrono::{Duration, Utc};
use sqlx::PgPool;

use crate::{
    error::{AppError, Result},
    models::{PhoneVerificationCode, UserPhoneNumber},
};

const CODE_EXPIRY_MINUTES: i64 = 5;
const RESEND_COOLDOWN_SECONDS: i64 = 30;
const MAX_CODES_PER_HOUR: i64 = 5;
const MAX_ATTEMPTS: i32 = 5;

const PHONE_COLUMNS: &str = "id, phone_number, verified_at IS NOT NULL AS verified, verified_at";

fn map_unique_violation(e: sqlx::Error) -> AppError {
    match e.as_database_error().and_then(|d| d.code()) {
        Some(code) if code == "23505" => {
            AppError::Conflict("ეს ნომერი უკვე დამატებულია".to_string())
        }
        _ => AppError::DatabaseError(e),
    }
}

pub async fn get_user_phone_numbers(pool: &PgPool, user_id: i32) -> Result<Vec<UserPhoneNumber>> {
    let phones = sqlx::query_as::<_, UserPhoneNumber>(&format!(
        "SELECT {PHONE_COLUMNS} FROM user_phone_numbers WHERE user_id = $1 ORDER BY created_at"
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(phones)
}

pub async fn add_user_phone_number(
    pool: &PgPool,
    user_id: i32,
    phone_number: &str,
) -> Result<UserPhoneNumber> {
    let phone = sqlx::query_as::<_, UserPhoneNumber>(&format!(
        "INSERT INTO user_phone_numbers (user_id, phone_number) VALUES ($1, $2) RETURNING {PHONE_COLUMNS}"
    ))
    .bind(user_id)
    .bind(phone_number)
    .fetch_one(pool)
    .await
    .map_err(map_unique_violation)?;

    Ok(phone)
}

pub async fn update_user_phone_number(
    pool: &PgPool,
    user_id: i32,
    id: i32,
    phone_number: &str,
) -> Result<Option<UserPhoneNumber>> {
    let phone = sqlx::query_as::<_, UserPhoneNumber>(&format!(
        "UPDATE user_phone_numbers
         SET verified_at = CASE WHEN phone_number = $1 THEN verified_at ELSE NULL END,
             phone_number = $1,
             updated_at = NOW()
         WHERE user_id = $2 AND id = $3
         RETURNING {PHONE_COLUMNS}"
    ))
    .bind(phone_number)
    .bind(user_id)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(map_unique_violation)?;

    Ok(phone)
}

pub async fn delete_user_phone_number(
    pool: &PgPool,
    user_id: i32,
    id: i32,
) -> Result<Option<UserPhoneNumber>> {
    let phone = sqlx::query_as::<_, UserPhoneNumber>(&format!(
        "DELETE FROM user_phone_numbers WHERE user_id = $1 AND id = $2 RETURNING {PHONE_COLUMNS}"
    ))
    .bind(user_id)
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(phone)
}

pub async fn is_phone_verified(pool: &PgPool, user_id: i32, phone_number: &str) -> Result<bool> {
    let verified = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1 FROM user_phone_numbers
            WHERE user_id = $1 AND phone_number = $2 AND verified_at IS NOT NULL
        )",
    )
    .bind(user_id)
    .bind(phone_number)
    .fetch_one(pool)
    .await?;

    Ok(verified)
}

pub async fn mark_phone_verified(
    pool: &PgPool,
    user_id: i32,
    phone_number: &str,
) -> Result<UserPhoneNumber> {
    let phone = sqlx::query_as::<_, UserPhoneNumber>(&format!(
        "INSERT INTO user_phone_numbers (user_id, phone_number, verified_at)
         VALUES ($1, $2, NOW())
         ON CONFLICT (user_id, phone_number)
         DO UPDATE SET verified_at = COALESCE(user_phone_numbers.verified_at, NOW()), updated_at = NOW()
         RETURNING {PHONE_COLUMNS}"
    ))
    .bind(user_id)
    .bind(phone_number)
    .fetch_one(pool)
    .await?;

    Ok(phone)
}

pub async fn create_verification_code(pool: &PgPool, phone_number: &str, code: i32) -> Result<()> {
    sqlx::query("DELETE FROM phone_verification_codes WHERE created_at < NOW() - INTERVAL '1 day'")
        .execute(pool)
        .await?;

    let (recent, last_hour): (bool, i64) = sqlx::query_as(
        "SELECT
            COALESCE(BOOL_OR(created_at > NOW() - make_interval(secs => $2)), false),
            COUNT(*)
         FROM phone_verification_codes
         WHERE phone_number = $1 AND created_at > NOW() - INTERVAL '1 hour'",
    )
    .bind(phone_number)
    .bind(RESEND_COOLDOWN_SECONDS as f64)
    .fetch_one(pool)
    .await?;

    if recent {
        return Err(AppError::TooManyRequests(format!(
            "კოდის ხელახლა გაგზავნა შესაძლებელია {RESEND_COOLDOWN_SECONDS} წამში"
        )));
    }

    if last_hour >= MAX_CODES_PER_HOUR {
        return Err(AppError::TooManyRequests(
            "ძალიან ბევრი მცდელობა, სცადეთ მოგვიანებით".to_string(),
        ));
    }

    sqlx::query(
        "INSERT INTO phone_verification_codes (phone_number, code, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(phone_number)
    .bind(code)
    .bind(Utc::now() + Duration::minutes(CODE_EXPIRY_MINUTES))
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn check_verification_code(pool: &PgPool, phone_number: &str, code: i32) -> Result<()> {
    let latest = sqlx::query_as::<_, PhoneVerificationCode>(
        "SELECT id, code, attempts FROM phone_verification_codes
         WHERE phone_number = $1 AND expires_at > NOW()
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(phone_number)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("კოდის ვადა ამოიწურა, მოითხოვეთ ახალი კოდი".to_string()))?;

    if latest.attempts >= MAX_ATTEMPTS {
        return Err(AppError::TooManyRequests(
            "ძალიან ბევრი არასწორი მცდელობა, მოითხოვეთ ახალი კოდი".to_string(),
        ));
    }

    if latest.code != code {
        sqlx::query("UPDATE phone_verification_codes SET attempts = attempts + 1 WHERE id = $1")
            .bind(latest.id)
            .execute(pool)
            .await?;
        return Err(AppError::BadRequest(
            "არასწორი დამადასტურებელი კოდი".to_string(),
        ));
    }

    Ok(())
}

pub async fn consume_verification_codes(pool: &PgPool, phone_number: &str) -> Result<()> {
    sqlx::query("DELETE FROM phone_verification_codes WHERE phone_number = $1")
        .bind(phone_number)
        .execute(pool)
        .await?;

    Ok(())
}
