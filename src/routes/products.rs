use axum::{
    Json,
    extract::{Path, Query, State},
};
use http::StatusCode;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{
        Brand, CableType, CableVariant, CableTypeWithVariants, ProductFacets, ProductQuery,
        ProductResponse, ProductSearchResponse, TopProductsQuery,
    },
    queries::{admin_queries, products_queries},
    utils::extractors::OptionalClaims,
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
    Path(id): Path<String>,
) -> Result<Json<ProductResponse>> {
    let (data, images, categories, seo) = products_queries::find_product_bundle(&state.db, &id)
        .await?
        .ok_or(AppError::NotFound("პროდუქტი ვერ მოიძებნა".to_string()))?;

    Ok(Json(ProductResponse {
        data,
        images,
        categories,
        seo,
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

pub async fn get_related_products(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ProductResponse>>> {
    let related = products_queries::get_related_products(&state.db, &id, 12).await?;
    Ok(Json(related))
}

pub async fn get_brands(State(state): State<AppState>) -> Result<Json<Vec<Brand>>> {
    let brands = admin_queries::get_brands(&state.db).await?;
    Ok(Json(brands))
}

pub async fn get_cable_types(
    State(state): State<AppState>,
) -> Result<Json<Vec<CableType>>> {
    let types = admin_queries::get_cable_types(&state.db).await?;
    Ok(Json(types))
}

pub async fn get_cable_type_with_variants(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<CableTypeWithVariants>> {
    let cable_type = admin_queries::find_cable_type_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("cable type id-ით {} ვერ მოიძებნა", id)))?;
    let variants = admin_queries::get_cable_variants_by_type(&state.db, id).await?;
    Ok(Json(CableTypeWithVariants {
        cable_type,
        variants,
    }))
}

pub async fn get_cable_variants(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<Vec<CableVariant>>> {
    if admin_queries::find_cable_type_by_id(&state.db, id)
        .await?
        .is_none()
    {
        return Err(AppError::NotFound(format!(
            "cable type id-ით {} ვერ მოიძებნა",
            id
        )));
    }
    let variants = admin_queries::get_cable_variants_by_type(&state.db, id).await?;
    Ok(Json(variants))
}

pub async fn add_product_views(
    State(state): State<AppState>,
    Path(id): Path<String>,
    OptionalClaims(claims): OptionalClaims,
) -> Result<StatusCode> {
    let user_id = claims.map(|c| c.user_id);
    products_queries::add_product_views(&state.db, &id, user_id).await?;

    Ok(StatusCode::CREATED)
}

pub async fn get_top_products(
    State(state): State<AppState>,
    Query(params): Query<TopProductsQuery>,
) -> Result<Json<Vec<ProductResponse>>> {
    let ids = admin_queries::get_top_product_ids(&state.db, params.limit).await?;
    let response = products_queries::build_products_response_ordered(&state.db, &ids).await?;
    Ok(Json(response))
}
