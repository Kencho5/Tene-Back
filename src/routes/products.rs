use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::{
    AppState,
    error::{AppError, Result},
    models::{ProductQuery, ProductResponse},
    queries::products_queries,
};

pub async fn search_product(
    State(state): State<AppState>,
    Query(params): Query<ProductQuery>,
) -> Result<Json<Vec<ProductResponse>>> {
    let products = products_queries::search_products(&state.db, params).await?;

    Ok(Json(products))
}

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
