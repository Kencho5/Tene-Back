use std::collections::HashMap;

use sqlx::PgPool;

use crate::{
    error::Result,
    models::{
        BrandFacetValue, Category, CategoryFacetValue, FacetValue, Product, ProductFacets,
        ProductImage, ProductQuery, ProductResponse, SaleType, SortBy,
    },
};

pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Product>> {
    let product = sqlx::query_as::<_, Product>(
        "SELECT p.*, b.name as brand_name FROM products p LEFT JOIN brands b ON p.brand_id = b.id WHERE p.id = $1"
    )
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
const DEFAULT_PAGE_SIZE: i64 = 15;
const MAX_PAGE_SIZE: i64 = 100;

pub async fn search_products(
    pool: &PgPool,
    params: ProductQuery,
) -> Result<crate::models::ProductSearchResponse> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0);

    // If ID is provided, search only by ID
    if let Some(id) = params.id {
        let mut query = String::from(
            "SELECT p.*, b.name as brand_name FROM products p LEFT JOIN brands b ON p.brand_id = b.id WHERE p.id = $1",
        );
        if let Some(enabled) = params.enabled {
            query.push_str(&format!(" AND p.enabled = {}", enabled));
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

                let categories = sqlx::query_as::<_, Category>(
                    "SELECT c.* FROM categories c
                     INNER JOIN product_categories pc ON c.id = pc.category_id
                     WHERE pc.product_id = $1
                     ORDER BY c.display_order ASC, c.name ASC",
                )
                .bind(id)
                .fetch_all(pool)
                .await?;

                Ok(crate::models::ProductSearchResponse {
                    products: vec![ProductResponse {
                        data: product,
                        images,
                        categories,
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

    let mut query_builder =
        sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT p.*, b.name as brand_name, ");

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
        .push(" as relevance_score, COUNT(*) OVER() as total_count FROM products p LEFT JOIN brands b ON p.brand_id = b.id WHERE 1=1");

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

    if let Some(brand_id) = params.brand {
        query_builder.push(" AND p.brand_id = ");
        query_builder.push_bind(brand_id);
    }

    if !params.color.is_empty() {
        query_builder.push(" AND EXISTS (SELECT 1 FROM product_images pi WHERE pi.product_id = p.id AND pi.color = ANY(");
        query_builder.push_bind(&params.color);
        query_builder.push("))");
    }

    if !params.category_id.is_empty() {
        query_builder.push(
            " AND EXISTS (
                WITH RECURSIVE category_tree AS (
                    SELECT id FROM categories WHERE id = ANY(",
        );
        query_builder.push_bind(&params.category_id);
        query_builder.push(
            ")
                    UNION ALL
                    SELECT c.id FROM categories c
                    INNER JOIN category_tree ct ON c.parent_id = ct.id
                )
                SELECT 1 FROM product_categories pc
                WHERE pc.product_id = p.id
                AND pc.category_id IN (SELECT id FROM category_tree)
            )",
        );
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

    // fetch categories
    #[derive(sqlx::FromRow)]
    struct ProductCategoryRow {
        product_id: i32,
        #[sqlx(flatten)]
        category: Category,
    }

    let categories = sqlx::query_as::<_, ProductCategoryRow>(
        "SELECT pc.product_id, c.*
         FROM product_categories pc
         INNER JOIN categories c ON pc.category_id = c.id
         WHERE pc.product_id = ANY($1)
         ORDER BY pc.product_id, c.display_order ASC, c.name ASC",
    )
    .bind(&product_ids)
    .fetch_all(pool)
    .await?;

    let mut category_groups: HashMap<i32, Vec<Category>> =
        categories
            .into_iter()
            .fold(HashMap::with_capacity(product_ids.len()), |mut acc, row| {
                acc.entry(row.product_id).or_default().push(row.category);
                acc
            });

    let products = results
        .into_iter()
        .map(|result| ProductResponse {
            images: image_groups.remove(&result.product.id).unwrap_or_default(),
            categories: category_groups
                .remove(&result.product.id)
                .unwrap_or_default(),
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
    let mut filter_builder =
        sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT p.id FROM products p WHERE 1=1");

    if let Some(enabled) = params.enabled {
        filter_builder.push(" AND p.enabled = ");
        filter_builder.push_bind(enabled);
    }

    if let Some(q) = &params.query {
        filter_builder.push(" AND (p.name ILIKE ");
        filter_builder.push_bind(format!("%{}%", q));
        filter_builder.push(" OR p.description ILIKE ");
        filter_builder.push_bind(format!("%{}%", q));
        filter_builder.push(" OR similarity(p.name, ");
        filter_builder.push_bind(q);
        filter_builder.push(") > 0.3 OR similarity(COALESCE(p.description, ''), ");
        filter_builder.push_bind(q);
        filter_builder.push(") > 0.3)");
    }

    if let Some(min_price) = params.price_from {
        filter_builder.push(" AND p.price >= ");
        filter_builder.push_bind(min_price);
    }
    if let Some(max_price) = params.price_to {
        filter_builder.push(" AND p.price <= ");
        filter_builder.push_bind(max_price);
    }

    if let Some(brand_id) = params.brand {
        filter_builder.push(" AND p.brand_id = ");
        filter_builder.push_bind(brand_id);
    }

    if !params.color.is_empty() {
        filter_builder.push(" AND EXISTS (SELECT 1 FROM product_images pi WHERE pi.product_id = p.id AND pi.color = ANY(");
        filter_builder.push_bind(&params.color);
        filter_builder.push("))");
    }

    if !params.category_id.is_empty() {
        filter_builder.push(
            " AND EXISTS (
                WITH RECURSIVE category_tree AS (
                    SELECT id FROM categories WHERE id = ANY(",
        );
        filter_builder.push_bind(&params.category_id);
        filter_builder.push(
            ")
                    UNION ALL
                    SELECT c.id FROM categories c
                    INNER JOIN category_tree ct ON c.parent_id = ct.id
                )
                SELECT 1 FROM product_categories pc
                WHERE pc.product_id = p.id
                AND pc.category_id IN (SELECT id FROM category_tree)
            )",
        );
    }

    let has_discount = params.sale_type.contains(&SaleType::Discount);
    let has_coins = params.sale_type.contains(&SaleType::Coins);
    if has_discount && !has_coins {
        filter_builder.push(" AND p.discount > 0");
    } else if !has_discount && has_coins {
        filter_builder.push(" AND false");
    }

    // First, get filtered product IDs
    let filtered_ids: Vec<i32> = filter_builder.build_query_scalar().fetch_all(pool).await?;

    if filtered_ids.is_empty() {
        return Ok(ProductFacets {
            brands: Vec::new(),
            colors: Vec::new(),
            categories: Vec::new(),
        });
    }

    // Query all three facets using the filtered IDs
    let brands = sqlx::query_as::<_, BrandFacetValue>(
        "SELECT b.id, b.name, COUNT(*)::bigint as count
         FROM products p
         JOIN brands b ON p.brand_id = b.id
         WHERE p.id = ANY($1)
         GROUP BY b.id, b.name
         ORDER BY count DESC
         LIMIT 50",
    )
    .bind(&filtered_ids)
    .fetch_all(pool)
    .await?;

    let colors = sqlx::query_as::<_, FacetValue>(
        "SELECT pi.color as value, COUNT(DISTINCT p.id)::bigint as count
         FROM product_images pi
         JOIN products p ON pi.product_id = p.id
         WHERE p.id = ANY($1) AND pi.color IS NOT NULL AND pi.color != ''
         GROUP BY pi.color
         ORDER BY count DESC
         LIMIT 50",
    )
    .bind(&filtered_ids)
    .fetch_all(pool)
    .await?;

    let categories = sqlx::query_as::<_, CategoryFacetValue>(
        "SELECT c.id, c.name, COUNT(DISTINCT p.id)::bigint as count
         FROM product_categories pc
         JOIN categories c ON pc.category_id = c.id
         JOIN products p ON pc.product_id = p.id
         WHERE p.id = ANY($1) AND c.enabled = true
         GROUP BY c.id, c.name
         ORDER BY count DESC
         LIMIT 100",
    )
    .bind(&filtered_ids)
    .fetch_all(pool)
    .await?;

    Ok(ProductFacets {
        brands,
        colors,
        categories,
    })
}
