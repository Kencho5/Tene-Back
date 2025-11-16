use axum::{extract::{Path, State}, Json};

use crate::{error::{AppError, Result}, models::Product, queries::product_queries, AppState};

pub async fn get_product(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<Product>> {
    let product = product_queries::find_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("Product not found".to_string()))?;

    Ok(Json(product))
}
