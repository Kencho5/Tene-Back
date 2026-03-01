use axum::{Json, extract::State, http::StatusCode};
use rand::Rng;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{SendVerificationCodeRequest, VerifyCodeRequest},
    queries::email_queries,
    services::email_service,
};

pub async fn send_verification_code(
    State(state): State<AppState>,
    Json(payload): Json<SendVerificationCodeRequest>,
) -> Result<StatusCode> {
    validate_email(&payload.email)?;

    let code = rand::rng().random_range(100000..999999);

    let sender_email = "noreply@example.com".to_string();

    email_queries::delete_codes_for_email(&state.db, &payload.email).await?;

    email_queries::create_verification_code(&state.db, &payload.email, code).await?;

    email_service::send_verification_email(&state.ses_client, &payload.email, code, &sender_email)
        .await?;

    tracing::info!("Verification code sent to {}", payload.email);

    Ok(StatusCode::OK)
}

pub async fn verify_code(
    State(state): State<AppState>,
    Json(payload): Json<VerifyCodeRequest>,
) -> Result<StatusCode> {
    let verification = email_queries::find_valid_code(&state.db, &payload.email, payload.code)
        .await?
        .ok_or_else(|| {
            AppError::Unauthorized("არასწორი ან ვადაგასული დამადასტურებელი კოდი".to_string())
        })?;

    email_queries::delete_code(&state.db, verification.id).await?;

    tracing::info!("Email verified for {}", payload.email);

    Ok(StatusCode::OK)
}

fn validate_email(email: &str) -> Result<()> {
    if email.is_empty() || !email.contains('@') {
        return Err(AppError::BadRequest("არასწორი ელფოსტის მისამართი".to_string()));
    }
    Ok(())
}
