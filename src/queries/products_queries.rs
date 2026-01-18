use sqlx::{PgPool, QueryBuilder, Postgres};

use crate::{
    error::Result,
    models::{Product, ProductImage, ProductQuery},
};

pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Product>> {
    let product = sqlx::query_as::<_, Product>("SELECT * FROM products WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(product)
}

pub async fn find_images_by_product_id(
    pool: &PgPool,
    id: i32,
) -> Result<Vec<ProductImage>> {
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

pub async fn search_products(pool: &PgPool, params: ProductQuery) -> Result<Vec<Product>> {
    let mut query: QueryBuilder<Postgres> = QueryBuilder::new("SELECT * FROM products WHERE 1=1");
    let mut has_text_search = false;

    if let Some(ref q) = params.query {
        has_text_search = true;
        query.push(" AND (name ILIKE ");
        query.push_bind(format!("%{}%", q));
        query.push(" OR description ILIKE ");
        query.push_bind(format!("%{}%", q));
        query.push(" OR similarity(name, ");
        query.push_bind(q);
        query.push(") > 0.3");
        query.push(" OR similarity(COALESCE(description, ''), ");
        query.push_bind(q);
        query.push(") > 0.3)");
    }

    if let Some(price_from) = params.price_from {
        query.push(" AND price >= ");
        query.push_bind(price_from);
    }

    if let Some(price_to) = params.price_to {
        query.push(" AND price <= ");
        query.push_bind(price_to);
    }

    if let Some(ref sale_type) = params.sale_type {
        query.push(" AND product_type = ");
        query.push_bind(sale_type);
    }

    if let Some(ref brand) = params.brand {
        query.push(" AND specifications->>'brand' ILIKE ");
        query.push_bind(format!("%{}%", brand));
    }

    if has_text_search {
        if let Some(ref q) = params.query {
            query.push(" ORDER BY GREATEST(similarity(name, ");
            query.push_bind(q);
            query.push("), similarity(COALESCE(description, ''), ");
            query.push_bind(q);
            query.push(")) DESC, created_at DESC");
        }
    } else {
        query.push(" ORDER BY created_at DESC");
    }

    let products = query.build_query_as::<Product>()
        .fetch_all(pool)
        .await?;

    Ok(products)
}
