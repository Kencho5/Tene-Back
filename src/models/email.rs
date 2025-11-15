use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SendVerificationCodeRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyCodeRequest {
    pub email: String,
    pub code: i32,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct VerificationCode {
    pub id: i32,
    pub email: String,
    pub code: i32,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
