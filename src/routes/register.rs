use axum::{Json, extract::State};

use crate::{
    AppState,
    error::{AppError, Result},
    models::{AuthResponse, RegisterRequest},
    queries::user_queries,
    utils::jwt,
};

pub async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>> {
    validate_registration(&payload)?;

    if user_queries::find_by_email(&state.db, &payload.email)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict("ელფოსტა უკვე რეგისტრირებულია".to_string()));
    }

    let password_hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::InternalError(format!("პაროლის ჰეშირება ვერ მოხერხდა: {}", e)))?;

    let user =
        user_queries::create_user(&state.db, &payload.email, &payload.name, &password_hash).await?;

    let token = jwt::generate_token(user.id, &user.email, &user.name, user.role)?;

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
