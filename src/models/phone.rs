use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserPhoneNumber {
    pub id: i32,
    pub phone_number: String,
    pub verified: bool,
    pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct PhoneNumberRequest {
    pub phone_number: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyPhoneRequest {
    pub phone_number: String,
    pub code: i32,
}

#[derive(Debug, sqlx::FromRow)]
pub struct PhoneVerificationCode {
    pub id: i32,
    pub code: i32,
    pub attempts: i32,
}
