use axum::{Extension, Json, extract::State};

use crate::{
    AppState,
    error::{AppError, Result},
    models::UserAddress,
    queries::user_queries::add_user_address,
    utils::jwt::Claims,
};

pub async fn add_address(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<UserAddress>,
) -> Result<Json<UserAddress>> {
    let user_id = claims
        .sub
        .parse::<i32>()
        .map_err(|_| AppError::Unauthorized("Unauthorized".to_string()))?;

    let address = add_user_address(&state.db, user_id, payload).await?;

    Ok(Json(address))
}
