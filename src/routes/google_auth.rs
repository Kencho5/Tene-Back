use axum::{Json, extract::State};
use google_oauth::AsyncClient;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{AuthResponse, GoogleAuthRequest},
    queries::user_queries,
    utils::jwt,
};

pub async fn google_auth(
    State(state): State<AppState>,
    Json(payload): Json<GoogleAuthRequest>,
) -> Result<Json<AuthResponse>> {
    let google_client_id = std::env::var("GOOGLE_CLIENT_ID")
        .map_err(|_| AppError::ConfigError("GOOGLE_CLIENT_ID not set".to_string()))?;

    let client = AsyncClient::new(&google_client_id);

    let payload_result = client
        .validate_id_token(&payload.id_token)
        .await
        .map_err(|e| AppError::BadRequest(format!("Invalid Google token: {}", e)))?;

    let google_id = &payload_result.sub;
    let email = payload_result
        .email
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Email not provided by Google".to_string()))?;
    let name = payload_result
        .name
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("Name not provided by Google".to_string()))?;

    let user =
        if let Some(existing_user) = user_queries::find_by_google_id(&state.db, google_id).await? {
            existing_user
        } else if let Some(existing_user) = user_queries::find_by_email(&state.db, email).await? {
            if existing_user.password.is_some() {
                return Err(AppError::Conflict(
                    "Email already registered with password. Please login with email/password"
                        .to_string(),
                ));
            }
            existing_user
        } else {
            user_queries::create_google_user(&state.db, email, name, google_id).await?
        };

    let token = jwt::generate_token(user.id, &user.email, &user.name, user.role)?;

    Ok(Json(AuthResponse { token }))
}
