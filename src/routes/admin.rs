use axum::{
    Json,
    extract::{Path, Query, State},
};

use http::StatusCode;
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{
        ImageMetadataUpdate, ImageUploadUrl, ProductImage, ProductImageUrlRequest,
        ProductImageUrlResponse, ProductRequest, ProductResponse, UserQuery, UserSearchResponse,
    },
    queries::{admin_queries, products_queries},
    services::image_url_service::{delete_objects_by_prefix, delete_single_object, put_object_url},
};

//PRODUCT ROUTES
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

    let product = admin_queries::create_product(&state.db, &payload).await?;
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

    let product = admin_queries::update_product(&state.db, id, &payload).await?;
    let images = products_queries::find_images_by_product_id(&state.db, product.id).await?;

    Ok(Json(ProductResponse {
        data: product,
        images,
    }))
}

pub async fn delete_product(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode> {
    if products_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound("Product not found".to_string()));
    }

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "products-staging",
        crate::config::Environment::Main => "products-main",
    };

    let s3_prefix = format!("{}/{}/", env_prefix, id);

    delete_objects_by_prefix(&state.s3_client, &state.s3_bucket, &s3_prefix)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to delete images from S3: {}", e)))?;

    admin_queries::delete_product(&state.db, id).await?;

    Ok(StatusCode::NO_CONTENT)
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

        admin_queries::add_product_image(
            &state.db,
            id,
            image_uuid,
            req.color,
            req.is_primary,
            extension,
        )
        .await?;

        responses.push(ImageUploadUrl {
            image_uuid,
            upload_url,
            public_url,
        });
    }

    Ok(Json(ProductImageUrlResponse { images: responses }))
}

pub async fn delete_product_image(
    State(state): State<AppState>,
    Path((product_id, image_uuid)): Path<(i32, Uuid)>,
) -> Result<StatusCode> {
    let deleted_image = admin_queries::delete_product_image(&state.db, product_id, image_uuid)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Image {} not found for product {}",
                image_uuid, product_id
            ))
        })?;

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "products-staging",
        crate::config::Environment::Main => "products-main",
    };

    let key = format!(
        "{}/{}/{}.{}",
        env_prefix, product_id, deleted_image.image_uuid, deleted_image.extension
    );

    delete_single_object(&state.s3_client, &state.s3_bucket, &key)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to delete image from S3: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn update_product_image_metadata(
    State(state): State<AppState>,
    Path((product_id, image_uuid)): Path<(i32, Uuid)>,
    Json(payload): Json<ImageMetadataUpdate>,
) -> Result<Json<ProductImage>> {
    if payload.color.is_none() && payload.is_primary.is_none() {
        return Err(AppError::BadRequest(
            "At least one field (color or is_primary) must be provided".to_string(),
        ));
    }

    let updated_image = admin_queries::update_product_image_metadata(
        &state.db,
        product_id,
        image_uuid,
        payload.color,
        payload.is_primary,
    )
    .await?
    .ok_or_else(|| {
        AppError::NotFound(format!(
            "Image {} not found for product {}",
            image_uuid, product_id
        ))
    })?;

    Ok(Json(updated_image))
}

//USER ROUTES
pub async fn search_users(
    State(state): State<AppState>,
    Query(params): Query<UserQuery>,
) -> Result<Json<UserSearchResponse>> {
    let response = admin_queries::search_users(&state.db, params).await?;

    Ok(Json(response))
}
