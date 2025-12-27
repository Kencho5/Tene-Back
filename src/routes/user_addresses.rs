use axum::{Extension, Json, extract::State};

use crate::{
    AppState,
    error::Result,
    models::UserAddress,
    queries::user_queries::{add_user_address, get_user_addresses},
    utils::{extractors::extract_user_id, jwt::Claims},
};

pub async fn add_address(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<UserAddress>,
) -> Result<Json<UserAddress>> {
    let user_id = extract_user_id(&claims)?;

    let address = add_user_address(&state.db, user_id, payload).await?;

    Ok(Json(address))
}

pub async fn get_address(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<UserAddress>>> {
    let user_id = extract_user_id(&claims)?;

    let addresses = get_user_addresses(&state.db, user_id).await?;

    Ok(Json(addresses))
}
