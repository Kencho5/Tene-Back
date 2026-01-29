use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::{
    AppState,
    error::{AppError, Result},
    models::{ProductFacets, ProductQuery, ProductResponse, ProductSearchResponse},
    queries::products_queries,
};

pub async fn search_product(
    State(state): State<AppState>,
    Query(mut params): Query<ProductQuery>,
) -> Result<Json<ProductSearchResponse>> {
    if params.enabled.is_none() {
        params.enabled = Some(true);
    }
    let response = products_queries::search_products(&state.db, params).await?;

    Ok(Json(response))
}

pub async fn get_product(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<ProductResponse>> {
    let data = products_queries::find_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("Product not found".to_string()))?;

    let images = products_queries::find_images_by_product_id(&state.db, id).await?;
    let categories = crate::queries::category_queries::get_product_categories(&state.db, id).await?;

    Ok(Json(ProductResponse {
        data,
        images,
        categories,
    }))
}

pub async fn get_product_facets(
    State(state): State<AppState>,
    Query(mut params): Query<ProductQuery>,
) -> Result<Json<ProductFacets>> {
    if params.enabled.is_none() {
        params.enabled = Some(true);
    }
    let facets = products_queries::get_product_facets(&state.db, params).await?;

    Ok(Json(facets))
}
