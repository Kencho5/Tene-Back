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
        "SELECT product_id, image_uuid, color, is_primary
         FROM product_images
         WHERE product_id = $1
         ORDER BY is_primary DESC, created_at ASC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(product_images)
}

const SIMILARITY_THRESHOLD: f64 = 0.3;
const DEFAULT_PAGE_SIZE: i64 = 6;
const MAX_PAGE_SIZE: i64 = 100;

pub async fn search_products(
    pool: &PgPool,
    params: ProductQuery,
) -> Result<crate::models::ProductSearchResponse> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0);

    let search_pattern = params.query.as_ref().map(|q| format!("%{}%", q));
    let sort_price_asc = matches!(params.sort_by, Some(SortBy::PriceAsc));
    let sort_price_desc = matches!(params.sort_by, Some(SortBy::PriceDesc));

    let has_discount_filter = params.sale_type.contains(&SaleType::Discount);
    let has_coins_filter = params.sale_type.contains(&SaleType::Coins);
    let colors_array: Vec<&str> = params.color.iter().map(|s| s.as_str()).collect();

    #[derive(sqlx::FromRow)]
    struct SearchResult {
        #[sqlx(flatten)]
        product: Product,
        total_count: i64,
    }

    let results = sqlx::query_as::<_, SearchResult>(
        r#"
        WITH filtered_products AS (
            SELECT
                p.*,
                CASE
                    WHEN $2::text IS NOT NULL THEN
                        GREATEST(
                            similarity(p.name, $2),
                            similarity(COALESCE(p.description, ''), $2)
                        )
                    ELSE 0
                END as relevance_score
            FROM products p
            WHERE
                ($1::text IS NULL OR p.name ILIKE $1 OR p.description ILIKE $1
                 OR similarity(p.name, $2) > $3
                 OR similarity(COALESCE(p.description, ''), $2) > $3)
                AND ($4::int2 IS NULL OR p.price >= $4)
                AND ($5::int2 IS NULL OR p.price <= $5)
                AND ($6::text IS NULL OR p.product_type = $6)
                AND ($7::text IS NULL OR p.brand = $7)
                AND (
                    CASE
                        WHEN ARRAY_LENGTH($8::text[], 1) IS NULL THEN true
                        ELSE EXISTS (
                            SELECT 1 FROM product_images pi
                            WHERE pi.product_id = p.id AND pi.color = ANY($8)
                        )
                    END
                )
                AND (
                    CASE
                        WHEN NOT $9::bool AND NOT $10::bool THEN true
                        WHEN $9::bool AND $10::bool THEN true
                        WHEN $9::bool THEN p.discount > 0
                        WHEN $10::bool THEN false
                        ELSE true
                    END
                )
        )
        SELECT
            *,
            COUNT(*) OVER() as total_count
        FROM filtered_products
        ORDER BY
            relevance_score DESC,
            CASE WHEN $11 = true THEN price END ASC,
            CASE WHEN $12 = true THEN price END DESC,
            created_at DESC
        LIMIT $13 OFFSET $14
        "#,
    )
    .bind(&search_pattern)
    .bind(params.query.as_ref())
    .bind(SIMILARITY_THRESHOLD)
    .bind(params.price_from)
    .bind(params.price_to)
    .bind(params.product_type.as_ref())
    .bind(params.brand.as_ref())
    .bind(&colors_array)
    .bind(has_discount_filter)
    .bind(has_coins_filter)
    .bind(sort_price_asc)
    .bind(sort_price_desc)
    .bind(limit)
    .bind(offset)
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

    let product_ids: Vec<i32> = results.iter().map(|r| r.product.id).collect();

    let images = sqlx::query_as::<_, ProductImage>(
        "SELECT product_id, image_uuid, color, is_primary
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
