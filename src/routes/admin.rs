use axum::{
    Json,
    extract::{Path, Query, State},
};

use http::StatusCode;
use uuid::Uuid;

use rust_decimal::{Decimal, prelude::ToPrimitive};

use crate::{
    AppState,
    error::{AppError, Result},
    models::*,
    queries::{admin_queries, category_queries, order_queries, products_queries, user_queries},
    services::{
        delivery_service, flitt_service,
        image_url_service::{delete_objects_by_prefix, delete_single_object, put_object_url},
    },
};

fn resolve_discount(
    price: Option<Decimal>,
    discount: Option<Decimal>,
    discounted_price: Option<Decimal>,
) -> Result<Option<Decimal>> {
    if discount.is_some() && discounted_price.is_some() {
        return Err(AppError::BadRequest(
            "discount და discounted_price ერთდროულად ვერ მიეთითება".to_string(),
        ));
    }

    if let Some(d) = discount {
        if d < Decimal::ZERO || d > Decimal::from(100) {
            return Err(AppError::BadRequest(
                "discount უნდა იყოს 0-დან 100-მდე".to_string(),
            ));
        }
        return Ok(Some(d));
    }

    if let Some(dp) = discounted_price {
        let p = price.ok_or_else(|| {
            AppError::BadRequest("discounted_price-ისთვის price აუცილებელია".to_string())
        })?;
        if p <= Decimal::ZERO {
            return Err(AppError::BadRequest("price უნდა იყოს დადებითი".to_string()));
        }
        if dp < Decimal::ZERO || dp > p {
            return Err(AppError::BadRequest(
                "discounted_price უნდა იყოს 0-დან price-მდე".to_string(),
            ));
        }
        let pct = (p - dp) * Decimal::from(100) / p;
        return Ok(Some(pct));
    }

    Ok(None)
}

// products
pub async fn create_product(
    State(state): State<AppState>,
    Json(mut payload): Json<ProductRequest>,
) -> Result<Json<ProductResponse>> {
    let id = payload
        .id
        .as_ref()
        .ok_or_else(|| AppError::BadRequest("id აუცილებელია".to_string()))?;

    if payload.name.is_none() {
        return Err(AppError::BadRequest("სახელი აუცილებელია".to_string()));
    }

    if payload.price.is_none() {
        return Err(AppError::BadRequest("ფასი აუცილებელია".to_string()));
    }

    if products_queries::find_by_id(&state.db, id).await?.is_some() {
        return Err(AppError::Conflict(format!(
            "პროდუქტი id-ით {} უკვე არსებობს",
            id
        )));
    }

    payload.discount = resolve_discount(payload.price, payload.discount, payload.discounted_price)?;

    if let Some(ref seo) = payload.seo {
        if let Some(ref slug) = seo.slug {
            if let Some(other_id) =
                admin_queries::find_product_seo_by_slug(&state.db, slug).await?
            {
                if &other_id != id {
                    return Err(AppError::Conflict(format!(
                        "slug '{}' უკვე გამოყენებულია",
                        slug
                    )));
                }
            }
        }
    }

    let product = admin_queries::create_product(&state.db, &payload).await?;

    let seo = if let Some(ref seo_req) = payload.seo {
        Some(admin_queries::upsert_product_seo(&state.db, &product.id, seo_req).await?)
    } else {
        None
    };

    let images = products_queries::find_images_by_product_id(&state.db, &product.id).await?;
    let categories = category_queries::get_product_categories(&state.db, &product.id).await?;

    Ok(Json(ProductResponse {
        data: product,
        images,
        categories,
        seo,
    }))
}

pub async fn update_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut payload): Json<ProductRequest>,
) -> Result<Json<ProductResponse>> {
    let existing = products_queries::find_by_id(&state.db, &id).await?.ok_or_else(|| {
        AppError::NotFound(format!("პროდუქტი id-ით {} ვერ მოიძებნა", id))
    })?;

    let effective_price = payload.price.or(Some(existing.price));
    payload.discount = resolve_discount(effective_price, payload.discount, payload.discounted_price)?;

    if let Some(ref seo) = payload.seo {
        if let Some(ref slug) = seo.slug {
            if let Some(other_id) =
                admin_queries::find_product_seo_by_slug(&state.db, slug).await?
            {
                if other_id != id {
                    return Err(AppError::Conflict(format!(
                        "slug '{}' უკვე გამოყენებულია",
                        slug
                    )));
                }
            }
        }
    }

    let product = admin_queries::update_product(&state.db, &id, &payload).await?;

    let seo = if let Some(ref seo_req) = payload.seo {
        Some(admin_queries::upsert_product_seo(&state.db, &product.id, seo_req).await?)
    } else {
        admin_queries::get_product_seo(&state.db, &product.id).await?
    };

    let images = products_queries::find_images_by_product_id(&state.db, &product.id).await?;
    let categories = category_queries::get_product_categories(&state.db, &product.id).await?;

    Ok(Json(ProductResponse {
        data: product,
        images,
        categories,
        seo,
    }))
}

pub async fn delete_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode> {
    if products_queries::find_by_id(&state.db, &id).await?.is_none() {
        return Err(AppError::NotFound("პროდუქტი ვერ მოიძებნა".to_string()));
    }

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "products-staging",
        crate::config::Environment::Main => "products-main",
    };

    let s3_prefix = format!("{}/{}/", env_prefix, id);

    delete_objects_by_prefix(&state.s3_client, &state.s3_bucket, &s3_prefix)
        .await
        .map_err(|e| AppError::InternalError(format!("S3-დან სურათების წაშლა ვერ მოხერხდა: {}", e)))?;

    admin_queries::delete_product(&state.db, &id).await?;

    Ok(StatusCode::NO_CONTENT)
}
pub async fn generate_product_urls(
    State(state): State<AppState>,
    Path(id): Path<String>,
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
            "public, max-age=31536000, immutable",
            900,
        )
        .await
        .map_err(|e| AppError::InternalError(format!("წინასწარ ხელმოწერილი URL-ის გენერაცია ვერ მოხერხდა: {}", e)))?;

        let public_url = format!("{}/{}", state.assets_url, key);

        admin_queries::add_product_image(
            &state.db,
            &id,
            image_uuid,
            req.color,
            req.is_primary,
            extension,
            req.quantity.unwrap_or(0),
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
    Path((product_id, image_uuid)): Path<(String, Uuid)>,
) -> Result<StatusCode> {
    let deleted_image = admin_queries::delete_product_image(&state.db, &product_id, image_uuid)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "სურათი {} ვერ მოიძებნა პროდუქტისთვის {}",
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
        .map_err(|e| AppError::InternalError(format!("S3-დან სურათის წაშლა ვერ მოხერხდა: {}", e)))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn update_product_image_metadata(
    State(state): State<AppState>,
    Path((product_id, image_uuid)): Path<(String, Uuid)>,
    Json(payload): Json<ImageMetadataUpdate>,
) -> Result<Json<ProductImage>> {
    if payload.color.is_none() && payload.is_primary.is_none() && payload.quantity.is_none() {
        return Err(AppError::BadRequest(
            "მინიმუმ ერთი ველი (ფერი, is_primary ან რაოდენობა) უნდა იყოს მითითებული".to_string(),
        ));
    }

    let updated_image = admin_queries::update_product_image_metadata(
        &state.db,
        &product_id,
        image_uuid,
        payload.color,
        payload.is_primary,
        payload.quantity,
    )
    .await?
    .ok_or_else(|| {
        AppError::NotFound(format!(
            "სურათი {} ვერ მოიძებნა პროდუქტისთვის {}",
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

// users
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
        return Err(AppError::NotFound(format!("მომხმარებელი id-ით {} ვერ მოიძებნა", id)));
    }

    let user = admin_queries::update_user(&state.db, id, &payload).await?;

    Ok(Json(user))
}

pub async fn delete_user(State(state): State<AppState>, Path(id): Path<i32>) -> Result<StatusCode> {
    if user_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!("მომხმარებელი id-ით {} ვერ მოიძებნა", id)));
    }

    admin_queries::delete_user(&state.db, id).await?;

    Ok(StatusCode::NO_CONTENT)
}

// categories
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

    fn collect_ids(nodes: &[crate::models::CategoryWithChildren], ids: &mut Vec<i32>) {
        for node in nodes {
            ids.push(node.category.id);
            collect_ids(&node.children, ids);
        }
    }

    let mut category_ids = Vec::new();
    collect_ids(&tree.categories, &mut category_ids);

    let images = category_queries::get_category_images(&state.db, &category_ids).await?;

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "categories-staging",
        crate::config::Environment::Main => "categories-main",
    };

    let build_image_url = |category_id: i32| -> Option<String> {
        images.get(&category_id).map(|img| {
            format!(
                "{}/{}/{}/{}.{}",
                state.assets_url, env_prefix, category_id, img.image_uuid, img.extension
            )
        })
    };

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
            "კატეგორია id-ით {} ვერ მოიძებნა",
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
            "კატეგორია slug-ით '{}' უკვე არსებობს",
            payload.slug
        )));
    }

    if let Some(parent_id) = payload.parent_id {
        if category_queries::find_by_id(&state.db, parent_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound(format!(
                "მშობელი კატეგორია id-ით {} ვერ მოიძებნა",
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
            "კატეგორია id-ით {} ვერ მოიძებნა",
            id
        )));
    }

    if let Some(ref new_slug) = payload.slug {
        if let Some(existing) = category_queries::find_by_slug(&state.db, new_slug).await? {
            if existing.id != id {
                return Err(AppError::Conflict(format!(
                    "სხვა კატეგორია slug-ით '{}' უკვე არსებობს",
                    new_slug
                )));
            }
        }
    }

    if let Some(parent_id) = payload.parent_id {
        if parent_id == id {
            return Err(AppError::BadRequest(
                "კატეგორია არ შეიძლება იყოს საკუთარი მშობელი".to_string(),
            ));
        }
        if category_queries::find_by_id(&state.db, parent_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound(format!(
                "მშობელი კატეგორია id-ით {} ვერ მოიძებნა",
                parent_id
            )));
        }
    }

    let category = category_queries::update_category(&state.db, id, payload)
        .await?
        .ok_or(AppError::NotFound(format!(
            "კატეგორია id-ით {} ვერ მოიძებნა",
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
            "კატეგორია id-ით {} ვერ მოიძებნა",
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
    Path(product_id): Path<String>,
    Json(payload): Json<AssignCategoriesRequest>,
) -> Result<StatusCode> {
    if products_queries::find_by_id(&state.db, &product_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!(
            "პროდუქტი id-ით {} ვერ მოიძებნა",
            product_id
        )));
    }

    for category_id in &payload.category_ids {
        if category_queries::find_by_id(&state.db, *category_id)
            .await?
            .is_none()
        {
            return Err(AppError::NotFound(format!(
                "კატეგორია id-ით {} ვერ მოიძებნა",
                category_id
            )));
        }
    }

    category_queries::assign_categories_to_product(&state.db, &product_id, &payload.category_ids)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn generate_category_image_url(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<CategoryImageUploadRequest>,
) -> Result<Json<CategoryImageUploadUrl>> {
    if category_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!(
            "კატეგორია id-ით {} ვერ მოიძებნა",
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

    if let Some(existing) = category_queries::get_category_image(&state.db, id).await? {
        let old_key = format!(
            "{}/{}/{}.{}",
            env_prefix, id, existing.image_uuid, existing.extension
        );
        delete_single_object(&state.s3_client, &state.s3_bucket, &old_key)
            .await
            .map_err(|e| AppError::InternalError(format!("S3-დან ძველი სურათის წაშლა ვერ მოხერხდა: {}", e)))?;
    }

    let key = format!("{}/{}/{}.{}", env_prefix, id, image_uuid, extension);

    let upload_url = put_object_url(
        &state.s3_client,
        &state.s3_bucket,
        &key,
        &payload.content_type,
        "public, max-age=31536000, immutable",
        900,
    )
    .await
    .map_err(|e| AppError::InternalError(format!("წინასწარ ხელმოწერილი URL-ის გენერაცია ვერ მოხერხდა: {}", e)))?;

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
    if category_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!(
            "კატეგორია id-ით {} ვერ მოიძებნა",
            id
        )));
    }

    let image = category_queries::get_category_image(&state.db, id)
        .await?
        .ok_or(AppError::NotFound("კატეგორიის სურათი ვერ მოიძებნა".to_string()))?;

    if image.image_uuid != image_uuid {
        return Err(AppError::NotFound("კატეგორიის სურათი ვერ მოიძებნა".to_string()));
    }

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "categories-staging",
        crate::config::Environment::Main => "categories-main",
    };

    let key = format!("{}/{}/{}.{}", env_prefix, id, image_uuid, image.extension);

    delete_single_object(&state.s3_client, &state.s3_bucket, &key)
        .await
        .map_err(|e| AppError::InternalError(format!("S3-დან სურათის წაშლა ვერ მოხერხდა: {}", e)))?;

    category_queries::delete_category_image(&state.db, id, image_uuid).await?;

    Ok(StatusCode::NO_CONTENT)
}

// brands
pub async fn get_brands(State(state): State<AppState>) -> Result<Json<Vec<Brand>>> {
    let brands = admin_queries::get_brands(&state.db).await?;
    Ok(Json(brands))
}

pub async fn create_brand(
    State(state): State<AppState>,
    Json(payload): Json<BrandRequest>,
) -> Result<Json<Brand>> {
    if admin_queries::find_brand_by_name(&state.db, &payload.name)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(format!(
            "ბრენდი '{}' უკვე არსებობს",
            payload.name
        )));
    }

    let brand = admin_queries::create_brand(&state.db, &payload.name).await?;
    Ok(Json(brand))
}

pub async fn update_brand(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<BrandRequest>,
) -> Result<Json<Brand>> {
    if admin_queries::find_brand_by_id(&state.db, id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!("ბრენდი id-ით {} ვერ მოიძებნა", id)));
    }

    if let Some(existing) = admin_queries::find_brand_by_name(&state.db, &payload.name).await? {
        if existing.id != id {
            return Err(AppError::Conflict(format!(
                "ბრენდი '{}' უკვე არსებობს",
                payload.name
            )));
        }
    }

    let brand = admin_queries::update_brand(&state.db, id, &payload.name).await?;
    Ok(Json(brand))
}

pub async fn delete_brand(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode> {
    if admin_queries::find_brand_by_id(&state.db, id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!("ბრენდი id-ით {} ვერ მოიძებნა", id)));
    }

    admin_queries::delete_brand(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// cable types
pub async fn get_cable_types(
    State(state): State<AppState>,
) -> Result<Json<Vec<CableTypeWithVariants>>> {
    let types = admin_queries::get_cable_types(&state.db).await?;
    let mut result = Vec::with_capacity(types.len());
    for t in types {
        let variants = admin_queries::get_cable_variants_by_type(&state.db, t.id).await?;
        result.push(CableTypeWithVariants {
            cable_type: t,
            variants,
        });
    }
    Ok(Json(result))
}

pub async fn create_cable_type(
    State(state): State<AppState>,
    Json(payload): Json<CableTypeRequest>,
) -> Result<Json<CableType>> {
    if payload.name.trim().is_empty() {
        return Err(AppError::BadRequest("name აუცილებელია".to_string()));
    }

    if admin_queries::find_cable_type_by_name(&state.db, &payload.name)
        .await?
        .is_some()
    {
        return Err(AppError::Conflict(format!(
            "cable type '{}' უკვე არსებობს",
            payload.name
        )));
    }

    let t = admin_queries::create_cable_type(&state.db, &payload).await?;
    Ok(Json(t))
}

pub async fn update_cable_type(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<CableTypeRequest>,
) -> Result<Json<CableType>> {
    if payload.name.trim().is_empty() {
        return Err(AppError::BadRequest("name აუცილებელია".to_string()));
    }
    if admin_queries::find_cable_type_by_id(&state.db, id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!(
            "cable type id-ით {} ვერ მოიძებნა",
            id
        )));
    }

    if let Some(existing) = admin_queries::find_cable_type_by_name(&state.db, &payload.name).await?
    {
        if existing.id != id {
            return Err(AppError::Conflict(format!(
                "cable type '{}' უკვე არსებობს",
                payload.name
            )));
        }
    }

    let t = admin_queries::update_cable_type(&state.db, id, &payload).await?;
    Ok(Json(t))
}

pub async fn delete_cable_type(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode> {
    if admin_queries::find_cable_type_by_id(&state.db, id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!(
            "cable type id-ით {} ვერ მოიძებნა",
            id
        )));
    }

    admin_queries::delete_cable_type(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// cable variants
pub async fn get_cable_variants(
    State(state): State<AppState>,
    Path(type_id): Path<i32>,
) -> Result<Json<Vec<CableVariant>>> {
    if admin_queries::find_cable_type_by_id(&state.db, type_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!(
            "cable type id-ით {} ვერ მოიძებნა",
            type_id
        )));
    }
    let variants = admin_queries::get_cable_variants_by_type(&state.db, type_id).await?;
    Ok(Json(variants))
}

pub async fn create_cable_variant(
    State(state): State<AppState>,
    Path(type_id): Path<i32>,
    Json(payload): Json<CableVariantRequest>,
) -> Result<Json<CableVariant>> {
    if admin_queries::find_cable_type_by_id(&state.db, type_id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!(
            "cable type id-ით {} ვერ მოიძებნა",
            type_id
        )));
    }
    if payload.watts <= 0 || payload.length_cm <= 0 {
        return Err(AppError::BadRequest(
            "watts და length_cm უნდა იყოს დადებითი".to_string(),
        ));
    }
    if payload.warranty_months < 0 {
        return Err(AppError::BadRequest(
            "warranty_months არ შეიძლება იყოს უარყოფითი".to_string(),
        ));
    }

    if admin_queries::find_cable_variant_by_combo(
        &state.db,
        type_id,
        payload.watts,
        payload.length_cm,
    )
    .await?
    .is_some()
    {
        return Err(AppError::Conflict(
            "ასეთი ვარიაცია უკვე არსებობს".to_string(),
        ));
    }

    let v = admin_queries::create_cable_variant(&state.db, type_id, &payload).await?;
    Ok(Json(v))
}

pub async fn update_cable_variant(
    State(state): State<AppState>,
    Path((type_id, variant_id)): Path<(i32, i32)>,
    Json(payload): Json<CableVariantUpdate>,
) -> Result<Json<CableVariant>> {
    let existing = admin_queries::find_cable_variant_by_id(&state.db, variant_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("ვარიაცია id-ით {} ვერ მოიძებნა", variant_id))
        })?;

    if existing.cable_type_id != type_id {
        return Err(AppError::NotFound(
            "ვარიაცია არ ეკუთვნის ამ cable type-ს".to_string(),
        ));
    }

    if payload.watts.is_none()
        && payload.length_cm.is_none()
        && payload.price.is_none()
        && payload.warranty_months.is_none()
    {
        return Err(AppError::BadRequest(
            "მინიმუმ ერთი ველი უნდა იყოს მითითებული".to_string(),
        ));
    }

    if let Some(w) = payload.watts {
        if w <= 0 {
            return Err(AppError::BadRequest("watts უნდა იყოს დადებითი".to_string()));
        }
    }
    if let Some(l) = payload.length_cm {
        if l <= 0 {
            return Err(AppError::BadRequest(
                "length_cm უნდა იყოს დადებითი".to_string(),
            ));
        }
    }
    if let Some(wm) = payload.warranty_months {
        if wm < 0 {
            return Err(AppError::BadRequest(
                "warranty_months არ შეიძლება იყოს უარყოფითი".to_string(),
            ));
        }
    }

    let new_watts = payload.watts.unwrap_or(existing.watts);
    let new_len = payload.length_cm.unwrap_or(existing.length_cm);

    if let Some(other) =
        admin_queries::find_cable_variant_by_combo(&state.db, type_id, new_watts, new_len).await?
    {
        if other.id != variant_id {
            return Err(AppError::Conflict(
                "ასეთი ვარიაცია უკვე არსებობს".to_string(),
            ));
        }
    }

    let v = admin_queries::update_cable_variant(&state.db, variant_id, &payload).await?;
    Ok(Json(v))
}

pub async fn delete_cable_variant(
    State(state): State<AppState>,
    Path((type_id, variant_id)): Path<(i32, i32)>,
) -> Result<StatusCode> {
    let existing = admin_queries::find_cable_variant_by_id(&state.db, variant_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!("ვარიაცია id-ით {} ვერ მოიძებნა", variant_id))
        })?;

    if existing.cable_type_id != type_id {
        return Err(AppError::NotFound(
            "ვარიაცია არ ეკუთვნის ამ cable type-ს".to_string(),
        ));
    }

    admin_queries::delete_cable_variant(&state.db, variant_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_orders(
    State(state): State<AppState>,
    Query(params): Query<OrderQuery>,
) -> Result<Json<OrderSearchResponse>> {
    let response = admin_queries::get_orders(&state.db, params).await?;
    Ok(Json(response))
}

pub async fn create_payment_link(
    State(state): State<AppState>,
    Json(payload): Json<PaymentLinkRequest>,
) -> Result<Json<CheckoutResponse>> {
    if payload.items.is_empty() {
        return Err(AppError::BadRequest("შეკვეთა ცარიელია".to_string()));
    }
    if payload.email.is_empty() || !payload.email.contains('@') {
        return Err(AppError::BadRequest("არასწორი ელფოსტა".to_string()));
    }

    let mut subtotal = Decimal::ZERO;
    let mut order_items = Vec::with_capacity(payload.items.len());
    for item in &payload.items {
        if item.quantity <= 0 {
            return Err(AppError::BadRequest(format!(
                "არასწორი რაოდენობა პროდუქტისთვის {}",
                item.product_id
            )));
        }
        if item.price < Decimal::ZERO {
            return Err(AppError::BadRequest(format!(
                "არასწორი ფასი პროდუქტისთვის {}",
                item.product_id
            )));
        }
        subtotal += item.price * Decimal::from(item.quantity);
        order_items.push(OrderItemData {
            product_id: item.product_id.clone(),
            color: item.color.clone(),
            quantity: item.quantity,
            price: item.price,
            product_name: item.product_name.clone(),
            image: serde_json::Value::Null,
            cable_config: None,
        });
    }

    let delivery = delivery_service::calculate_delivery(
        &payload.delivery_type,
        &payload.delivery_time,
        payload.city.as_deref(),
    )?;

    let amount_tetri = ((subtotal + delivery) * Decimal::from(100))
        .trunc()
        .to_i32()
        .ok_or_else(|| AppError::InternalError("თანხის გამოთვლა ვერ მოხერხდა".to_string()))?;

    if amount_tetri <= 0 {
        return Err(AppError::BadRequest(
            "შეკვეთის თანხა უნდა იყოს დადებითი".to_string(),
        ));
    }

    let order_id = format!("tene_{}", Uuid::new_v4());

    let contact = order_queries::OrderContact {
        customer: &payload.customer,
        email: &payload.email,
        phone_number: &payload.phone_number,
        address: &payload.address,
        city: payload.city.as_deref(),
        details: payload.details.as_deref(),
        delivery_type: &payload.delivery_type,
        delivery_time: &payload.delivery_time,
        comment: payload.comment.as_deref(),
    };

    order_queries::create_order_with_items_raw(
        &state.db,
        None,
        &order_id,
        amount_tetri,
        &contact,
        &order_items,
    )
    .await?;

    let server_callback_url = format!("{}/payments/callback", state.backend_url);
    let response_url = format!("{}/payments/redirect", state.backend_url);

    let checkout_url = flitt_service::create_checkout_url(
        state.flitt_merchant_id,
        &state.flitt_secret_key,
        &order_id,
        amount_tetri,
        &format!("Tene order {}", order_id),
        &server_callback_url,
        &response_url,
    )
    .await?;

    order_queries::update_order_checkout_url(&state.db, &order_id, &checkout_url).await?;

    Ok(Json(CheckoutResponse {
        order_id,
        checkout_url,
    }))
}

pub async fn get_analytics(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsQuery>,
) -> Result<Json<AnalyticsResponse>> {
    let analytics = admin_queries::get_analytics(&state.db, params).await?;
    Ok(Json(analytics))
}

pub async fn get_checkout_sessions(
    State(state): State<AppState>,
    Query(params): Query<CheckoutSessionQuery>,
) -> Result<Json<CheckoutSessionsResponse>> {
    let sessions = admin_queries::get_checkout_sessions(&state.db, params).await?;
    Ok(Json(sessions))
}

// top products
pub async fn get_top_products_admin(
    State(state): State<AppState>,
    Query(params): Query<TopProductsQuery>,
) -> Result<Json<Vec<ProductResponse>>> {
    let ids = admin_queries::get_top_product_ids(&state.db, params.limit).await?;
    let response = products_queries::build_products_response_ordered(&state.db, &ids).await?;
    Ok(Json(response))
}

pub async fn replace_top_products(
    State(state): State<AppState>,
    Json(payload): Json<TopProductsRequest>,
) -> Result<StatusCode> {
    let mut seen = std::collections::HashSet::new();
    for id in &payload.product_ids {
        if !seen.insert(id) {
            return Err(AppError::BadRequest(format!(
                "დუბლირებული პროდუქტი id-ით {}",
                id
            )));
        }
    }

    if !payload.product_ids.is_empty() {
        let found = products_queries::find_by_ids(&state.db, &payload.product_ids).await?;
        for id in &payload.product_ids {
            if !found.contains_key(id) {
                return Err(AppError::NotFound(format!(
                    "პროდუქტი id-ით {} ვერ მოიძებნა",
                    id
                )));
            }
        }
    }

    admin_queries::replace_top_products(&state.db, &payload.product_ids).await?;
    Ok(StatusCode::NO_CONTENT)
}

