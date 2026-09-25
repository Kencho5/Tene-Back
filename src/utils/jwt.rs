use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use std::env;

use crate::error::{AppError, Result};
use crate::models::UserRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub user_id: i32,
    pub email: String,
    pub name: String,
    pub role: UserRole,
    pub exp: usize,
}

pub fn generate_token(
    user_id: i32,
    email: &str,
    name: &str,
    role: UserRole,
    duration: chrono::Duration,
) -> Result<String> {
    let jwt_secret = env::var("JWT_SECRET")
        .map_err(|_| AppError::ConfigError("JWT_SECRET not set".to_string()))?;

    let expiration = chrono::Utc::now()
        .checked_add_signed(duration)
        .ok_or_else(|| AppError::InternalError("ვადის გამოთვლა ვერ მოხერხდა".to_string()))?
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        user_id,
        email: email.to_string(),
        name: name.to_string(),
        role,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::InternalError(format!("ტოკენის გენერაცია ვერ მოხერხდა: {}", e)))
}

pub fn verify_token(token: &str) -> Result<Claims> {
    let jwt_secret = env::var("JWT_SECRET")
        .map_err(|_| AppError::ConfigError("JWT_SECRET not set".to_string()))?;

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::BadRequest(format!("არასწორი ტოკენი: {}", e)))
}

const PHONE_VERIFICATION_PURPOSE: &str = "phone_verification";

#[derive(Debug, Serialize, Deserialize)]
struct PhoneVerificationClaims {
    phone_number: String,
    purpose: String,
    exp: usize,
}

pub fn generate_phone_verification_token(
    phone_number: &str,
    duration: chrono::Duration,
) -> Result<String> {
    let jwt_secret = env::var("JWT_SECRET")
        .map_err(|_| AppError::ConfigError("JWT_SECRET not set".to_string()))?;

    let expiration = chrono::Utc::now()
        .checked_add_signed(duration)
        .ok_or_else(|| AppError::InternalError("ვადის გამოთვლა ვერ მოხერხდა".to_string()))?
        .timestamp() as usize;

    let claims = PhoneVerificationClaims {
        phone_number: phone_number.to_string(),
        purpose: PHONE_VERIFICATION_PURPOSE.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::InternalError(format!("ტოკენის გენერაცია ვერ მოხერხდა: {}", e)))
}

pub fn verify_phone_verification_token(token: &str, phone_number: &str) -> Result<()> {
    let jwt_secret = env::var("JWT_SECRET")
        .map_err(|_| AppError::ConfigError("JWT_SECRET not set".to_string()))?;

    let invalid = || {
        AppError::PhoneVerificationRequired(
            "ტელეფონის დადასტურების ვადა ამოიწურა, დაადასტურეთ ნომერი თავიდან".to_string(),
        )
    };

    let claims = decode::<PhoneVerificationClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| invalid())?
    .claims;

    if claims.purpose != PHONE_VERIFICATION_PURPOSE || claims.phone_number != phone_number {
        return Err(invalid());
    }

    Ok(())
}
