use axum::{extract::State, Json};

use crate::{
    error::{AppError, Result},
    models::{AuthResponse, RegisterRequest},
    queries::user_queries,
    utils::jwt,
    AppState,
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
        return Err(AppError::Conflict("Email already registered".to_string()));
    }

    let password_hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
        .map_err(|e| AppError::InternalError(format!("Password hashing failed: {}", e)))?;

    let user =
        user_queries::create_user(&state.db, &payload.email, &payload.name, &password_hash).await?;

    let token = jwt::generate_token(user.id, &user.email, &user.name, user.role)?;

    Ok(Json(AuthResponse { token }))
}

fn validate_registration(payload: &RegisterRequest) -> Result<()> {
    if payload.email.is_empty() || !payload.email.contains('@') {
        return Err(AppError::BadRequest("Invalid email address".to_string()));
    }

    if payload.name.trim().is_empty() {
        return Err(AppError::BadRequest("Name cannot be empty".to_string()));
    }

    if payload.password.len() < 4 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    Ok(())
}
