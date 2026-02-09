use axum::{
    Json,
    extract::{Path, Query, State},
};

use http::StatusCode;
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, Result},
    models::*,
    queries::{admin_queries, category_queries, products_queries, user_queries},
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

    if products_queries::find_by_id(&state.db, id).await?.is_some() {
        return Err(AppError::Conflict(format!(
            "Product with id {} already exists",
            id
        )));
    }

    let product = admin_queries::create_product(&state.db, &payload).await?;
    let images = products_queries::find_images_by_product_id(&state.db, product.id).await?;
    let categories = category_queries::get_product_categories(&state.db, product.id).await?;

    Ok(Json(ProductResponse {
        data: product,
        images,
        categories,
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
    let categories = category_queries::get_product_categories(&state.db, product.id).await?;

    Ok(Json(ProductResponse {
        data: product,
        images,
        categories,
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

pub async fn search_products(
    State(state): State<AppState>,
    Query(params): Query<ProductQuery>,
) -> Result<Json<ProductSearchResponse>> {
    let response = products_queries::search_products(&state.db, params).await?;

    Ok(Json(response))
}

//USER ROUTES
pub async fn search_users(
    State(state): State<AppState>,
    Query(params): Query<UserQuery>,
) -> Result<Json<UserSearchResponse>> {
    let response = admin_queries::search_users(&state.db, params).await?;

    Ok(Json(response))
}

pub async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<UserRequest>,
) -> Result<Json<UserResponse>> {
    if user_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!("User with id {} not found", id)));
    }

    let user = admin_queries::update_user(&state.db, id, &payload).await?;

    Ok(Json(user))
}

pub async fn delete_user(State(state): State<AppState>, Path(id): Path<i32>) -> Result<StatusCode> {
    if user_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!("User with id {} not found", id)));
    }

    admin_queries::delete_user(&state.db, id).await?;

    Ok(StatusCode::NO_CONTENT)
}

//CATEGORY ROUTES
pub async fn get_all_categories_admin(
    State(state): State<AppState>,
) -> Result<Json<Vec<CategoryResponse>>> {
    let categories = category_queries::get_all(&state.db, false).await?;

    let category_ids: Vec<i32> = categories.iter().map(|c| c.id).collect();
    let images = category_queries::get_category_images(&state.db, &category_ids).await?;

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "categories-staging",
        crate::config::Environment::Main => "categories-main",
    };

    let response: Vec<CategoryResponse> = categories
        .into_iter()
        .map(|category| {
            let image_url = images.get(&category.id).map(|img| {
                format!(
                    "{}/{}/{}/{}.{}",
                    state.assets_url, env_prefix, category.id, img.image_uuid, img.extension
                )
            });

            CategoryResponse {
                category,
                image_url,
            }
        })
        .collect();

    Ok(Json(response))
}

pub async fn get_category_tree_admin(
    State(state): State<AppState>,
) -> Result<Json<CategoryTreeResponse>> {
    let tree = category_queries::get_category_tree(&state.db, false).await?;

    // Collect all category IDs from the tree
    fn collect_ids(nodes: &[crate::models::CategoryWithChildren], ids: &mut Vec<i32>) {
        for node in nodes {
            ids.push(node.category.id);
            collect_ids(&node.children, ids);
        }
    }

    let mut category_ids = Vec::new();
    collect_ids(&tree.categories, &mut category_ids);

    // Fetch all images
    let images = category_queries::get_category_images(&state.db, &category_ids).await?;

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "categories-staging",
        crate::config::Environment::Main => "categories-main",
    };

    // Build image URL helper
    let build_image_url = |category_id: i32| -> Option<String> {
        images.get(&category_id).map(|img| {
            format!(
                "{}/{}/{}/{}.{}",
                state.assets_url, env_prefix, category_id, img.image_uuid, img.extension
            )
        })
    };

    // Convert tree to response with image URLs
    fn build_response_tree(
        nodes: Vec<crate::models::CategoryWithChildren>,
        build_url: &dyn Fn(i32) -> Option<String>,
    ) -> Vec<CategoryResponseWithChildren> {
        nodes
            .into_iter()
            .map(|node| CategoryResponseWithChildren {
                image_url: build_url(node.category.id),
                children: build_response_tree(node.children, build_url),
                category: node.category,
            })
            .collect()
    }

    let response_categories = build_response_tree(tree.categories, &build_image_url);

    Ok(Json(CategoryTreeResponse {
        categories: response_categories,
    }))
}

pub async fn get_category(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<CategoryResponse>> {
    let category = category_queries::find_by_id(&state.db, id)
        .await?
        .ok_or(AppError::NotFound(format!(
            "Category with id {} not found",
            id
        )))?;

    let image = category_queries::get_category_image(&state.db, id).await?;

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "categories-staging",
        crate::config::Environment::Main => "categories-main",
    };

    let image_url = image.map(|img| {
        format!(
            "{}/{}/{}/{}.{}",
            state.assets_url, env_prefix, id, img.image_uuid, img.extension
        )
    });

    Ok(Json(CategoryResponse {
        category,
        image_url,
    }))
}

pub async fn create_category(
    State(state): State<AppState>,
    Json(payload): Json<CreateCategoryRequest>,
) -> Result<Json<Category>> {
    if category_queries::find_by_slug(&state.db, &payload.slug)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(format!(
            "Category with slug '{}' already exists",
            payload.slug
        )));
    }

    if let Some(parent_id) = payload.parent_id {
        if category_queries::find_by_id(&state.db, parent_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound(format!(
                "Parent category with id {} not found",
                parent_id
            )));
        }
    }

    let category = category_queries::create_category(&state.db, payload).await?;
    Ok(Json(category))
}

pub async fn update_category(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateCategoryRequest>,
) -> Result<Json<Category>> {
    if category_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!(
            "Category with id {} not found",
            id
        )));
    }

    if let Some(ref new_slug) = payload.slug {
        if let Some(existing) = category_queries::find_by_slug(&state.db, new_slug).await? {
            if existing.id != id {
                return Err(AppError::Conflict(format!(
                    "Another category with slug '{}' already exists",
                    new_slug
                )));
            }
        }
    }

    if let Some(parent_id) = payload.parent_id {
        if parent_id == id {
            return Err(AppError::BadRequest(
                "Category cannot be its own parent".to_string(),
            ));
        }
        if category_queries::find_by_id(&state.db, parent_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound(format!(
                "Parent category with id {} not found",
                parent_id
            )));
        }
    }

    let category = category_queries::update_category(&state.db, id, payload)
        .await?
        .ok_or(AppError::NotFound(format!(
            "Category with id {} not found",
            id
        )))?;

    Ok(Json(category))
}

pub async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode> {
    if category_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!(
            "Category with id {} not found",
            id
        )));
    }

    category_queries::delete_category(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
pub struct AssignCategoriesRequest {
    pub category_ids: Vec<i32>,
}

pub async fn assign_categories_to_product(
    State(state): State<AppState>,
    Path(product_id): Path<i32>,
    Json(payload): Json<AssignCategoriesRequest>,
) -> Result<StatusCode> {
    if products_queries::find_by_id(&state.db, product_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!(
            "Product with id {} not found",
            product_id
        )));
    }

    for category_id in &payload.category_ids {
        if category_queries::find_by_id(&state.db, *category_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound(format!(
                "Category with id {} not found",
                category_id
            )));
        }
    }

    category_queries::assign_categories_to_product(&state.db, product_id, &payload.category_ids)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn generate_category_image_url(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<CategoryImageUploadRequest>,
) -> Result<Json<CategoryImageUploadUrl>> {
    // Check if category exists
    if category_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!(
            "Category with id {} not found",
            id
        )));
    }

    let image_uuid = Uuid::new_v4();
    let extension = match payload.content_type.as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => "jpg",
    };

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "categories-staging",
        crate::config::Environment::Main => "categories-main",
    };

    let key = format!("{}/{}/{}.{}", env_prefix, id, image_uuid, extension);

    let upload_url = put_object_url(
        &state.s3_client,
        &state.s3_bucket,
        &key,
        &payload.content_type,
        900,
    )
    .await
    .map_err(|e| AppError::InternalError(format!("Failed to generate presigned URL: {}", e)))?;

    let public_url = format!("{}/{}", state.assets_url, key);

    category_queries::add_category_image(&state.db, id, image_uuid, extension).await?;

    Ok(Json(CategoryImageUploadUrl {
        image_uuid,
        upload_url,
        public_url,
    }))
}

pub async fn delete_category_image(
    State(state): State<AppState>,
    Path((id, image_uuid)): Path<(i32, Uuid)>,
) -> Result<StatusCode> {
    // Check if category exists
    if category_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!(
            "Category with id {} not found",
            id
        )));
    }

    // Get the image to find its extension
    let image = category_queries::get_category_image(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("Category image not found".to_string()))?;

    if image.image_uuid != image_uuid {
        return Err(AppError::NotFound("Category image not found".to_string()));
    }

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "categories-staging",
        crate::config::Environment::Main => "categories-main",
    };

    let key = format!("{}/{}/{}.{}", env_prefix, id, image_uuid, image.extension);

    delete_single_object(&state.s3_client, &state.s3_bucket, &key)
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to delete image from S3: {}", e)))?;

    category_queries::delete_category_image(&state.db, id, image_uuid).await?;

    Ok(StatusCode::NO_CONTENT)
}
