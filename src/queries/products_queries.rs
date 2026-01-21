use std::collections::HashMap;

use sqlx::{PgPool, Postgres, QueryBuilder};

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

pub async fn search_products(pool: &PgPool, params: ProductQuery) -> Result<Vec<ProductResponse>> {
    let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM products WHERE 1=1");
    let has_text_search = params.query.is_some();

    // text search
    if let Some(ref q) = params.query {
        query.push(" AND (name ILIKE ");
        query.push_bind(format!("%{}%", q));
        query.push(" OR description ILIKE ");
        query.push_bind(format!("%{}%", q));
        query.push(" OR similarity(name, ");
        query.push_bind(q);
        query.push(") > ");
        query.push_bind(SIMILARITY_THRESHOLD);
        query.push(" OR similarity(COALESCE(description, ''), ");
        query.push_bind(q);
        query.push(") > ");
        query.push_bind(SIMILARITY_THRESHOLD);
        query.push(")");
    }

    // price range
    if let Some(price_from) = params.price_from {
        query.push(" AND price >= ");
        query.push_bind(price_from);
    }

    if let Some(price_to) = params.price_to {
        query.push(" AND price <= ");
        query.push_bind(price_to);
    }

    // category
    if let Some(ref product_type) = params.product_type {
        query.push(" AND product_type = ");
        query.push_bind(product_type);
    }

    // brand
    if let Some(ref brand) = params.brand {
        query.push(" AND brand = ");
        query.push_bind(brand);
    }

    // color
    if let Some(ref color) = params.color {
        query.push(" AND EXISTS (SELECT 1 FROM product_images WHERE product_id = products.id AND color = ");
        query.push_bind(color);
        query.push(")");
    }

    // sale type
    if let Some(ref sale_type) = params.sale_type {
        match sale_type {
            SaleType::Discount => {
                query.push(" AND discount > 0");
            }
            SaleType::Coins => {
                // TODO: add coin_price column
            }
        }
    }

    // sort
    query.push(" ORDER BY ");

    if has_text_search {
        if let Some(ref q) = params.query {
            query.push("GREATEST(similarity(name, ");
            query.push_bind(q);
            query.push("), similarity(COALESCE(description, ''), ");
            query.push_bind(q);
            query.push(")) DESC");

            match params.sort_by {
                Some(SortBy::PriceAsc) => {
                    query.push(", price ASC");
                }
                Some(SortBy::PriceDesc) => {
                    query.push(", price DESC");
                }
                None => {
                    query.push(", created_at DESC");
                }
            }
        }
    } else {
        match params.sort_by {
            Some(SortBy::PriceAsc) => {
                query.push("price ASC");
            }
            Some(SortBy::PriceDesc) => {
                query.push("price DESC");
            }
            None => {
                query.push("created_at DESC");
            }
        }
    }

    // pagination
    let limit = params
        .limit
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .min(MAX_PAGE_SIZE);

    query.push(" LIMIT ");
    query.push_bind(limit);

    if let Some(offset) = params.offset {
        query.push(" OFFSET ");
        query.push_bind(offset);
    }

    let products = query.build_query_as::<Product>().fetch_all(pool).await?;

    if products.is_empty() {
        return Ok(Vec::new());
    }

    let product_ids: Vec<i32> = products.iter().map(|p| p.id).collect();

    let all_images = sqlx::query_as::<_, ProductImage>(
        "SELECT product_id, image_uuid, color, is_primary
         FROM product_images
         WHERE product_id = ANY($1)
         ORDER BY product_id, is_primary DESC, created_at ASC",
    )
    .bind(&product_ids)
    .fetch_all(pool)
    .await?;

    let mut images_map: HashMap<i32, Vec<ProductImage>> = HashMap::new();
    for image in all_images {
        images_map
            .entry(image.product_id)
            .or_insert_with(Vec::new)
            .push(image);
    }

    let result: Vec<ProductResponse> = products
        .into_iter()
        .map(|product| {
            let images = images_map.remove(&product.id).unwrap_or_default();
            ProductResponse { product, images }
        })
        .collect();

    Ok(result)
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
