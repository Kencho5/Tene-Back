use axum::{Json, extract::State};

use crate::{
    AppState,
    error::{AppError, Result},
    models::{AuthResponse, LoginRequest, UserRole},
    queries::user_queries,
    utils::jwt,
};

pub async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    let user = user_queries::find_by_email(&state.db, &payload.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("არასწორი ელფოსტა ან პაროლი".to_string()))?;

    let password_hash = user
        .password
        .as_ref()
        .ok_or_else(|| AppError::Unauthorized("არასწორი ელფოსტა ან პაროლი".to_string()))?;

    let is_valid = bcrypt::verify(&payload.password, password_hash)
        .map_err(|e| AppError::InternalError(format!("პაროლის შემოწმება ვერ მოხერხდა: {}", e)))?;

    if !is_valid {
        return Err(AppError::Unauthorized(
            "არასწორი ელფოსტა ან პაროლი".to_string(),
        ));
    }

    let duration = match user.role {
        UserRole::Admin => chrono::Duration::hours(3),
        _ => chrono::Duration::days(30),
    };

    let token = jwt::generate_token(user.id, &user.email, &user.name, user.role, duration)?;

    Ok(Json(AuthResponse { token }))
}
