use axum::{extract::State, Json};
use google_oauth::AsyncClient;

use crate::{
    error::{AppError, Result},
    models::{GoogleAuthRequest, RegisterResponse},
    queries::user_queries,
    AppState,
};

pub async fn google_auth(
    State(state): State<AppState>,
    Json(payload): Json<GoogleAuthRequest>,
) -> Result<Json<RegisterResponse>> {
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

    if let Some(existing_user) = user_queries::find_by_google_id(&state.db, google_id).await? {
        return Ok(Json(RegisterResponse::from(existing_user)));
    }

    if let Some(existing_user) = user_queries::find_by_email(&state.db, email).await? {
        if existing_user.password.is_some() {
            return Err(AppError::Conflict(
                "Email already registered with password. Please login with email/password"
                    .to_string(),
            ));
        }
        return Ok(Json(RegisterResponse::from(existing_user)));
    }

    let user = user_queries::create_google_user(&state.db, email, name, google_id).await?;

    Ok(Json(RegisterResponse::from(user)))
}
