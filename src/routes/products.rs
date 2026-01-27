use axum::{
    Json,
    extract::{Path, Query, State},
};

use crate::{
    AppState,
    error::{AppError, Result},
    models::{ProductFacets, ProductQuery, ProductRequest, ProductResponse, ProductSearchResponse},
    queries::products_queries,
};

pub async fn search_product(
    State(state): State<AppState>,
    Query(params): Query<ProductQuery>,
) -> Result<Json<ProductSearchResponse>> {
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

    Ok(Json(ProductResponse { data, images }))
}

pub async fn get_product_facets(
    State(state): State<AppState>,
    Query(params): Query<ProductQuery>,
) -> Result<Json<ProductFacets>> {
    let facets = products_queries::get_product_facets(&state.db, params).await?;

    Ok(Json(facets))
}

pub async fn create_product(
    State(state): State<AppState>,
    Json(payload): Json<ProductRequest>,
) -> Result<Json<ProductResponse>> {
    let id = payload
        .id
        .ok_or_else(|| AppError::BadRequest("id is required".to_string()))?;

    if payload.name.is_none() {
        return Err(AppError::BadRequest("name is required".to_string()));
    }

    if payload.price.is_none() {
        return Err(AppError::BadRequest("price is required".to_string()));
    }

    if payload.product_type.is_none() {
        return Err(AppError::BadRequest("product_type is required".to_string()));
    }

    if products_queries::find_by_id(&state.db, id).await?.is_some() {
        return Err(AppError::Conflict(format!(
            "Product with id {} already exists",
            id
        )));
    }

    let product = products_queries::create_product(&state.db, &payload).await?;
    let images = products_queries::find_images_by_product_id(&state.db, product.id).await?;

    Ok(Json(ProductResponse {
        data: product,
        images,
    }))
}

pub async fn update_product(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<ProductRequest>,
) -> Result<Json<ProductResponse>> {
    if products_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!(
            "Product with id {} not found",
            id
        )));
    }

    let product = products_queries::update_product(&state.db, id, &payload).await?;
    let images = products_queries::find_images_by_product_id(&state.db, product.id).await?;

    Ok(Json(ProductResponse {
        data: product,
        images,
    }))
}
