use axum::{
    Json,
    extract::{Path, Query, State},
};

use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{
        ImageUploadUrl, ProductFacets, ProductImageUrlRequest, ProductImageUrlResponse,
        ProductQuery, ProductRequest, ProductResponse, ProductSearchResponse,
    },
    queries::products_queries,
    services::image_url_service::put_object_url,
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

pub async fn generate_product_urls(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<ProductImageUrlRequest>,
) -> Result<Json<ProductImageUrlResponse>> {
    let mut responses = Vec::new();

    for req in payload.images {
        let image_uuid = Uuid::new_v4();
        let extension = match req.content_type.as_str() {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            _ => "jpg",
        };

        let env_prefix = match state.environment {
            crate::config::Environment::Staging => "products-staging",
            crate::config::Environment::Main => "products-main",
        };

        let key = format!("{}/{}/{}.{}", env_prefix, id, image_uuid, extension);

        let upload_url = put_object_url(
            &state.s3_client,
            &state.s3_bucket,
            &key,
            &req.content_type,
            900,
        )
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to generate presigned URL: {}", e)))?;

        let public_url = format!("{}/{}", state.assets_url, key);

        products_queries::add_product_image(&state.db, id, image_uuid, req.color, req.is_primary)
            .await?;

        responses.push(ImageUploadUrl {
            image_uuid,
            upload_url,
            public_url,
        });
    }

    Ok(Json(ProductImageUrlResponse { images: responses }))
}
