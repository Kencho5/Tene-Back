use axum::{
    Json,
    extract::{Path, State},
};

use crate::{
    AppState,
    error::{AppError, Result},
    models::ProductResponse,
    queries::products_queries,
};

pub async fn get_product(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<ProductResponse>> {
    let product = products_queries::find_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("Product not found".to_string()))?;

    let images = products_queries::find_images_by_product_id(&state.db, id).await?;

    Ok(Json(ProductResponse { product, images }))
}
