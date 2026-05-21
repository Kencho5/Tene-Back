use axum::{Json, extract::State, http::StatusCode};
use rand::Rng;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{AuthResponse, RegisterRequest, VerifyAndRegisterRequest},
    queries::{email_queries, user_queries},
    services::email_service,
    utils::jwt,
};

const SENDER_EMAIL: &str = "Tene <support@tene.ge>";

pub async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<StatusCode> {
    validate_registration(&payload)?;

    if user_queries::find_by_email(&state.db, &payload.email)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict("ელფოსტა უკვე რეგისტრირებულია".to_string()));
    }

    let code = rand::rng().random_range(100000..999999);

    email_queries::delete_codes_for_email(&state.db, &payload.email).await?;
    email_queries::create_verification_code(&state.db, &payload.email, code).await?;

    email_service::send_verification_email(&state.ses_client, &payload.email, code, SENDER_EMAIL)
        .await?;

    tracing::info!("Registration code sent to {}", payload.email);

    Ok(StatusCode::OK)
}

pub async fn verify_and_register(
    State(state): State<AppState>,
    Json(payload): Json<VerifyAndRegisterRequest>,
) -> Result<Json<AuthResponse>> {
    validate_registration(&RegisterRequest {
        email: payload.email.clone(),
        name: payload.name.clone(),
        password: payload.password.clone(),
    })?;

    if user_queries::find_by_email(&state.db, &payload.email)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict("ელფოსტა უკვე რეგისტრირებულია".to_string()));
    }

    let verification = email_queries::find_valid_code(&state.db, &payload.email, payload.code)
        .await?
        .ok_or_else(|| {
            AppError::Unauthorized("არასწორი ან ვადაგასული დამადასტურებელი კოდი".to_string())
        })?;

    email_queries::delete_code(&state.db, verification.id).await?;

    let password_hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::InternalError(format!("პაროლის ჰეშირება ვერ მოხერხდა: {}", e)))?;

    let user =
        user_queries::create_user(&state.db, &payload.email, &payload.name, &password_hash).await?;

    let token = jwt::generate_token(
        user.id,
        &user.email,
        &user.name,
        user.role,
        chrono::Duration::days(30),
    )?;

    Ok(Json(AuthResponse { token }))
}

fn validate_registration(payload: &RegisterRequest) -> Result<()> {
    if payload.email.is_empty() || !payload.email.contains('@') {
        return Err(AppError::BadRequest("არასწორი ელფოსტის მისამართი".to_string()));
    }

    if payload.name.trim().is_empty() {
        return Err(AppError::BadRequest("სახელი არ შეიძლება იყოს ცარიელი".to_string()));
    }

    if payload.password.len() < 4 {
        return Err(AppError::BadRequest(
            "პაროლი უნდა შეიცავდეს მინიმუმ 4 სიმბოლოს".to_string(),
        ));
    }

    Ok(())
}
