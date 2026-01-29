use std::collections::HashMap;

use sqlx::PgPool;

use crate::{
    error::Result,
    models::{
        FacetValue, Product, ProductFacets, ProductImage, ProductQuery, ProductResponse, SaleType,
        SortBy,
    },
};

pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Product>> {
    let product = sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(product)
}

pub async fn find_images_by_product_id(pool: &PgPool, id: i32) -> Result<Vec<ProductImage>> {
    let product_images = sqlx::query_as::<_, ProductImage>(
        "SELECT product_id, image_uuid, color, is_primary, extension
         FROM product_images
         WHERE product_id = $1
         ORDER BY is_primary DESC, created_at ASC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(product_images)
}

const SIMILARITY_THRESHOLD: f64 = 0.25;
const DEFAULT_PAGE_SIZE: i64 = 6;
const MAX_PAGE_SIZE: i64 = 100;

pub async fn search_products(
    pool: &PgPool,
    params: ProductQuery,
) -> Result<crate::models::ProductSearchResponse> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0);

    // If ID is provided, search only by ID
    if let Some(id) = params.id {
        let mut query = String::from("SELECT * FROM products WHERE id = $1");
        if let Some(enabled) = params.enabled {
            query.push_str(&format!(" AND enabled = {}", enabled));
        }

        let product = sqlx::query_as::<_, Product>(&query)
            .bind(id)
            .fetch_optional(pool)
            .await?;

        return match product {
            Some(product) => {
                let images = sqlx::query_as::<_, ProductImage>(
                    "SELECT product_id, image_uuid, color, is_primary, extension
                     FROM product_images
                     WHERE product_id = $1
                     ORDER BY is_primary DESC, created_at ASC",
                )
                .bind(id)
                .fetch_all(pool)
                .await?;

                Ok(crate::models::ProductSearchResponse {
                    products: vec![ProductResponse {
                        data: product,
                        images,
                    }],
                    total: 1,
                    limit,
                    offset,
                })
            }
            None => Ok(crate::models::ProductSearchResponse {
                products: Vec::new(),
                total: 0,
                limit,
                offset,
            }),
        };
    }

    let mut query_builder = sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT p.*, ");

    // calc relevance
    if let Some(q) = &params.query {
        query_builder.push("GREATEST(similarity(p.name, ");
        query_builder.push_bind(q);
        query_builder.push("), similarity(COALESCE(p.description, ''), ");
        query_builder.push_bind(q);
        query_builder.push("))");
    } else {
        query_builder.push("0");
    }
    query_builder
        .push(" as relevance_score, COUNT(*) OVER() as total_count FROM products p WHERE 1=1");

    if let Some(enabled) = params.enabled {
        query_builder.push(" AND p.enabled = ");
        query_builder.push_bind(enabled);
    }

    if let Some(q) = &params.query {
        query_builder.push(" AND (p.name ILIKE ");
        query_builder.push_bind(format!("%{}%", q));
        query_builder.push(" OR p.description ILIKE ");
        query_builder.push_bind(format!("%{}%", q));

        query_builder.push(" OR similarity(p.name, ");
        query_builder.push_bind(q);
        query_builder.push(") > ");
        query_builder.push_bind(SIMILARITY_THRESHOLD);

        query_builder.push(" OR similarity(COALESCE(p.description, ''), ");
        query_builder.push_bind(q);
        query_builder.push(") > ");
        query_builder.push_bind(SIMILARITY_THRESHOLD);
        query_builder.push(")");
    }

    if let Some(min_price) = params.price_from {
        query_builder.push(" AND p.price >= ");
        query_builder.push_bind(min_price);
    }
    if let Some(max_price) = params.price_to {
        query_builder.push(" AND p.price <= ");
        query_builder.push_bind(max_price);
    }

    if let Some(pt) = &params.product_type {
        query_builder.push(" AND p.product_type = ");
        query_builder.push_bind(pt);
    }

    if let Some(brand) = &params.brand {
        query_builder.push(" AND p.brand = ");
        query_builder.push_bind(brand);
    }

    if !params.color.is_empty() {
        query_builder.push(" AND EXISTS (SELECT 1 FROM product_images pi WHERE pi.product_id = p.id AND pi.color = ANY(");
        query_builder.push_bind(&params.color);
        query_builder.push("))");
    }

    let has_discount = params.sale_type.contains(&SaleType::Discount);
    let has_coins = params.sale_type.contains(&SaleType::Coins);

    if has_discount && !has_coins {
        query_builder.push(" AND p.discount > 0");
    } else if !has_discount && has_coins {
        query_builder.push(" AND false");
    }

    query_builder.push(" ORDER BY relevance_score DESC");

    match params.sort_by {
        Some(SortBy::PriceAsc) => query_builder.push(", p.price ASC"),
        Some(SortBy::PriceDesc) => query_builder.push(", p.price DESC"),
        None => &mut query_builder,
    };

    query_builder.push(", p.created_at DESC");

    query_builder.push(" LIMIT ");
    query_builder.push_bind(limit);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset);

    #[derive(sqlx::FromRow)]
    struct SearchResult {
        #[sqlx(flatten)]
        product: Product,
        total_count: i64,
    }

    let results = query_builder
        .build_query_as::<SearchResult>()
        .fetch_all(pool)
        .await?;

    let total = results.first().map(|r| r.total_count).unwrap_or(0);

    if results.is_empty() {
        return Ok(crate::models::ProductSearchResponse {
            products: Vec::new(),
            total,
            limit,
            offset,
        });
    }

    // fetch images
    let product_ids: Vec<i32> = results.iter().map(|r| r.product.id).collect();

    let images = sqlx::query_as::<_, ProductImage>(
        "SELECT product_id, image_uuid, color, is_primary, extension
         FROM product_images
         WHERE product_id = ANY($1)
         ORDER BY product_id, is_primary DESC, created_at ASC",
    )
    .bind(&product_ids)
    .fetch_all(pool)
    .await?;

    let mut image_groups: HashMap<i32, Vec<ProductImage>> =
        images
            .into_iter()
            .fold(HashMap::with_capacity(product_ids.len()), |mut acc, img| {
                acc.entry(img.product_id).or_default().push(img);
                acc
            });

    let products = results
        .into_iter()
        .map(|result| ProductResponse {
            images: image_groups.remove(&result.product.id).unwrap_or_default(),
            data: result.product,
        })
        .collect();

    Ok(crate::models::ProductSearchResponse {
        products,
        total,
        limit,
        offset,
    })
}

pub async fn get_product_facets(pool: &PgPool, params: ProductQuery) -> Result<ProductFacets> {
    let mut where_conditions = String::from("WHERE 1=1");
    if let Some(enabled) = params.enabled {
        where_conditions.push_str(&format!(" AND enabled = {}", enabled));
    }
    let mut bindings: Vec<String> = Vec::new();

    if let Some(ref q) = params.query {
        where_conditions.push_str(" AND (name ILIKE $1 OR description ILIKE $1 OR similarity(name, $1) > 0.3 OR similarity(COALESCE(description, ''), $1) > 0.3)");
        bindings.push(format!("%{}%", q));
    }

    let brands_query = format!(
        "SELECT
            brand as value,
            COUNT(*)::bigint as count
         FROM products
         {}
         AND brand IS NOT NULL
         AND brand != ''
         GROUP BY brand
         ORDER BY count DESC
         LIMIT 50",
        where_conditions
    );

    let colors_query = format!(
        "SELECT
            pi.color as value,
            COUNT(DISTINCT p.id)::bigint as count
         FROM products p
         JOIN product_images pi ON p.id = pi.product_id
         {}
         AND pi.color IS NOT NULL
         AND pi.color != ''
         GROUP BY pi.color
         ORDER BY count DESC
         LIMIT 50",
        where_conditions
    );

    let mut brands_q = sqlx::query_as::<_, FacetValue>(&brands_query);
    let mut colors_q = sqlx::query_as::<_, FacetValue>(&colors_query);

    for binding in &bindings {
        brands_q = brands_q.bind(binding);
        colors_q = colors_q.bind(binding);
    }

    let brands = brands_q.fetch_all(pool).await?;
    let colors = colors_q.fetch_all(pool).await?;

    Ok(ProductFacets { brands, colors })
}
