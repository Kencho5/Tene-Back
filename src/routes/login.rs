use axum::{extract::State, Json};

use crate::{
    error::{AppError, Result},
    models::{AuthResponse, LoginRequest},
    queries::user_queries,
    utils::jwt,
    AppState,
};

pub async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    let user = user_queries::find_by_email(&state.db, &payload.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("Invalid email or password".to_string()))?;

    let password_hash = user
        .password
        .as_ref()
        .ok_or_else(|| AppError::Unauthorized("Invalid email or password".to_string()))?;

    let is_valid = bcrypt::verify(&payload.password, password_hash)
        .map_err(|e| AppError::InternalError(format!("Password verification failed: {}", e)))?;

    if !is_valid {
        return Err(AppError::Unauthorized("Invalid email or password".to_string()));
    }

    let token = jwt::generate_token(user.id, &user.email)?;

    Ok(Json(AuthResponse { token }))
}
