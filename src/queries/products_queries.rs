use std::collections::HashMap;

use sqlx::PgPool;

use crate::{
    error::Result,
    models::{
        BrandFacetValue, Category, CategoryFacetValue, FacetValue, Product, ProductFacets,
        ProductImage, ProductQuery, ProductResponse, SaleType, SortBy,
    },
};

pub async fn find_by_id(pool: &PgPool, id: &str) -> Result<Option<Product>> {
    let product = sqlx::query_as::<_, Product>(
        "SELECT p.*, b.name as brand_name FROM products p LEFT JOIN brands b ON p.brand_id = b.id WHERE p.id = $1"
    )
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(product)
}

pub async fn find_images_by_product_id(pool: &PgPool, id: &str) -> Result<Vec<ProductImage>> {
    let product_images = sqlx::query_as::<_, ProductImage>(
        "SELECT product_id, image_uuid, color, is_primary, extension, quantity
         FROM product_images
         WHERE product_id = $1
         ORDER BY is_primary DESC, created_at ASC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(product_images)
}

pub async fn find_by_ids(pool: &PgPool, ids: &[String]) -> Result<HashMap<String, Product>> {
    let products = sqlx::query_as::<_, Product>(
        "SELECT p.*, b.name as brand_name
         FROM products p LEFT JOIN brands b ON p.brand_id = b.id
         WHERE p.id = ANY($1)",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;

    Ok(products.into_iter().map(|p| (p.id.clone(), p)).collect())
}

pub async fn find_images_by_product_ids(
    pool: &PgPool,
    ids: &[String],
) -> Result<HashMap<String, Vec<ProductImage>>> {
    let images = sqlx::query_as::<_, ProductImage>(
        "SELECT product_id, image_uuid, color, is_primary, extension, quantity
         FROM product_images
         WHERE product_id = ANY($1)
         ORDER BY product_id, is_primary DESC, created_at ASC",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;

    let mut groups: HashMap<String, Vec<ProductImage>> = HashMap::new();
    for img in images {
        groups.entry(img.product_id.clone()).or_default().push(img);
    }
    Ok(groups)
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

const SIMILARITY_THRESHOLD: f64 = 0.25;
const DEFAULT_PAGE_SIZE: i64 = 12;
const MAX_PAGE_SIZE: i64 = 100;

pub async fn search_products(
    pool: &PgPool,
    params: ProductQuery,
) -> Result<crate::models::ProductSearchResponse> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0);

    // If ID is provided, search only by ID
    if let Some(ref id) = params.id {
        let product = match params.enabled {
            Some(enabled) => {
                sqlx::query_as::<_, Product>(
                    "SELECT p.*, b.name as brand_name FROM products p LEFT JOIN brands b ON p.brand_id = b.id WHERE p.id = $1 AND p.enabled = $2",
                )
                .bind(id)
                .bind(enabled)
                .fetch_optional(pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, Product>(
                    "SELECT p.*, b.name as brand_name FROM products p LEFT JOIN brands b ON p.brand_id = b.id WHERE p.id = $1",
                )
                .bind(id)
                .fetch_optional(pool)
                .await?
            }
        };

        return match product {
            Some(product) => {
                let images = sqlx::query_as::<_, ProductImage>(
                    "SELECT product_id, image_uuid, color, is_primary, extension, quantity
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

    if params.in_stock == Some(true) {
        query_builder.push(" AND p.quantity > 0");
    }

    if let Some(q) = &params.query {
        let like_q = format!("%{}%", escape_like(q));
        query_builder.push(" AND (p.name ILIKE ");
        query_builder.push_bind(like_q.clone());
        query_builder.push(" OR p.description ILIKE ");
        query_builder.push_bind(like_q);

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

    let all_category_ids: Vec<i32> = params
        .parent_category_id
        .iter()
        .chain(params.child_category_id.iter())
        .copied()
        .collect();

    if !all_category_ids.is_empty() {
        query_builder.push(
            " AND EXISTS (
                WITH RECURSIVE category_tree AS (
                    SELECT id FROM categories WHERE id = ANY(",
        );
        query_builder.push_bind(&all_category_ids);
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

    match params.sort_by {
        Some(SortBy::PriceAsc) => {
            query_builder.push(" ORDER BY p.price ASC, relevance_score DESC, p.created_at DESC");
        }
        Some(SortBy::PriceDesc) => {
            query_builder.push(" ORDER BY p.price DESC, relevance_score DESC, p.created_at DESC");
        }
        None => {
            query_builder.push(" ORDER BY relevance_score DESC, p.created_at DESC");
        }
    };

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
    let product_ids: Vec<String> = results.iter().map(|r| r.product.id.clone()).collect();

    let images = sqlx::query_as::<_, ProductImage>(
        "SELECT product_id, image_uuid, color, is_primary, extension, quantity
         FROM product_images
         WHERE product_id = ANY($1)
         ORDER BY product_id, is_primary DESC, created_at ASC",
    )
    .bind(&product_ids)
    .fetch_all(pool)
    .await?;

    let mut image_groups: HashMap<String, Vec<ProductImage>> =
        images
            .into_iter()
            .fold(HashMap::with_capacity(product_ids.len()), |mut acc, img| {
                acc.entry(img.product_id.clone()).or_default().push(img);
                acc
            });

    // fetch categories
    #[derive(sqlx::FromRow)]
    struct ProductCategoryRow {
        product_id: String,
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

    let mut category_groups: HashMap<String, Vec<Category>> =
        categories
            .into_iter()
            .fold(HashMap::with_capacity(product_ids.len()), |mut acc, row| {
                acc.entry(row.product_id.clone()).or_default().push(row.category);
                acc
            });

    let products = results
        .into_iter()
        .map(|result| ProductResponse {
            images: image_groups.remove(&result.product.id).unwrap_or_default(),
            categories: category_groups.remove(&result.product.id).unwrap_or_default(),
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

pub async fn get_related_products(
    pool: &PgPool,
    product_id: &str,
    limit: i64,
) -> Result<Vec<ProductResponse>> {
    let products = sqlx::query_as::<_, Product>(
        "SELECT p.*, b.name as brand_name
         FROM products p
         LEFT JOIN brands b ON p.brand_id = b.id
         WHERE p.id != $1
           AND p.enabled = true
           AND EXISTS (
               SELECT 1 FROM product_categories pc
               WHERE pc.product_id = p.id
               AND pc.category_id IN (
                   SELECT category_id FROM product_categories WHERE product_id = $1
               )
           )
         ORDER BY (
             SELECT COUNT(*) FROM product_categories pc
             WHERE pc.product_id = p.id
             AND pc.category_id IN (
                 SELECT category_id FROM product_categories WHERE product_id = $1
             )
         ) DESC, p.created_at DESC
         LIMIT $2",
    )
    .bind(product_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    if products.is_empty() {
        return Ok(Vec::new());
    }

    let product_ids: Vec<String> = products.iter().map(|p| p.id.clone()).collect();
    let mut image_groups = find_images_by_product_ids(pool, &product_ids).await?;

    let responses = products
        .into_iter()
        .map(|product| {
            let images = image_groups.remove(&product.id).unwrap_or_default();
            ProductResponse {
                data: product,
                images,
                categories: Vec::new(),
            }
        })
        .collect();

    Ok(responses)
}

pub async fn get_product_facets(pool: &PgPool, params: ProductQuery) -> Result<ProductFacets> {
    let mut filter_builder =
        sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT p.id FROM products p WHERE 1=1");

    if let Some(enabled) = params.enabled {
        filter_builder.push(" AND p.enabled = ");
        filter_builder.push_bind(enabled);
    }

    if let Some(q) = &params.query {
        let like_q = format!("%{}%", escape_like(q));
        filter_builder.push(" AND (p.name ILIKE ");
        filter_builder.push_bind(like_q.clone());
        filter_builder.push(" OR p.description ILIKE ");
        filter_builder.push_bind(like_q);
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

    if !params.color.is_empty() {
        filter_builder.push(" AND EXISTS (SELECT 1 FROM product_images pi WHERE pi.product_id = p.id AND pi.color = ANY(");
        filter_builder.push_bind(&params.color);
        filter_builder.push("))");
    }

    let has_discount = params.sale_type.contains(&SaleType::Discount);
    let has_coins = params.sale_type.contains(&SaleType::Coins);
    if has_discount && !has_coins {
        filter_builder.push(" AND p.discount > 0");
    } else if !has_discount && has_coins {
        filter_builder.push(" AND false");
    }

    // Apply parent category filter to base query (affects all facets including category facet)
    if !params.parent_category_id.is_empty() {
        filter_builder.push(
            " AND EXISTS (
                WITH RECURSIVE category_tree AS (
                    SELECT id FROM categories WHERE id = ANY(",
        );
        filter_builder.push_bind(&params.parent_category_id);
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

    // Base IDs: without brand or child-category filters (for brand & category facets)
    let base_ids: Vec<String> = filter_builder.build_query_scalar().fetch_all(pool).await?;

    if base_ids.is_empty() {
        return Ok(ProductFacets {
            brands: Vec::new(),
            colors: Vec::new(),
            categories: Vec::new(),
        });
    }

    // Apply child-category filter on top of base IDs
    let category_filtered_ids = if !params.child_category_id.is_empty() {
        sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT p.id FROM products p
             WHERE p.id = ANY($1)
             AND EXISTS (
                 WITH RECURSIVE category_tree AS (
                     SELECT id FROM categories WHERE id = ANY($2)
                     UNION ALL
                     SELECT c.id FROM categories c
                     INNER JOIN category_tree ct ON c.parent_id = ct.id
                 )
                 SELECT 1 FROM product_categories pc
                 WHERE pc.product_id = p.id
                 AND pc.category_id IN (SELECT id FROM category_tree)
             )",
        )
        .bind(&base_ids)
        .bind(&params.child_category_id)
        .fetch_all(pool)
        .await?
    } else {
        base_ids.clone()
    };

    // Apply brand filter for fully filtered IDs (colors use this)
    let filtered_ids = if let Some(brand_id) = params.brand {
        sqlx::query_scalar::<_, String>(
            "SELECT p.id FROM products p WHERE p.id = ANY($1) AND p.brand_id = $2",
        )
        .bind(&category_filtered_ids)
        .bind(brand_id)
        .fetch_all(pool)
        .await?
    } else {
        category_filtered_ids.clone()
    };

    // Brand facet: base IDs + child-category filter, but NO brand filter
    let brands = sqlx::query_as::<_, BrandFacetValue>(
        "SELECT b.id, b.name, COUNT(*)::bigint as count
         FROM products p
         JOIN brands b ON p.brand_id = b.id
         WHERE p.id = ANY($1)
         GROUP BY b.id, b.name
         ORDER BY count DESC
         LIMIT 50",
    )
    .bind(&category_filtered_ids)
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

    // Category facet: base IDs + brand filter, but NO child-category filter
    let category_facet_ids = if let Some(brand_id) = params.brand {
        sqlx::query_scalar::<_, String>(
            "SELECT p.id FROM products p WHERE p.id = ANY($1) AND p.brand_id = $2",
        )
        .bind(&base_ids)
        .bind(brand_id)
        .fetch_all(pool)
        .await?
    } else {
        base_ids
    };

    let categories = if !params.parent_category_id.is_empty() {
        // When parent categories are selected, only show their descendants
        let mut cat_builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
            "SELECT c.id, c.parent_id, c.name, COUNT(DISTINCT p.id)::bigint as count
             FROM product_categories pc
             JOIN categories c ON pc.category_id = c.id
             JOIN products p ON pc.product_id = p.id
             WHERE p.id = ANY(",
        );
        cat_builder.push_bind(&category_facet_ids);
        cat_builder.push(
            ") AND c.enabled = true
             AND c.id IN (
                 WITH RECURSIVE category_tree AS (
                     SELECT id FROM categories WHERE id = ANY(",
        );
        cat_builder.push_bind(&params.parent_category_id);
        cat_builder.push(
            ")
                     UNION ALL
                     SELECT ch.id FROM categories ch
                     INNER JOIN category_tree ct ON ch.parent_id = ct.id
                 )
                 SELECT id FROM category_tree
             )
             GROUP BY c.id, c.parent_id, c.name
             ORDER BY count DESC
             LIMIT 100",
        );
        cat_builder
            .build_query_as::<CategoryFacetValue>()
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query_as::<_, CategoryFacetValue>(
            "SELECT c.id, c.parent_id, c.name, COUNT(DISTINCT p.id)::bigint as count
             FROM product_categories pc
             JOIN categories c ON pc.category_id = c.id
             JOIN products p ON pc.product_id = p.id
             WHERE p.id = ANY($1) AND c.enabled = true
             GROUP BY c.id, c.parent_id, c.name
             ORDER BY count DESC
             LIMIT 100",
        )
        .bind(&category_facet_ids)
        .fetch_all(pool)
        .await?
    };

    Ok(ProductFacets {
        brands,
        colors,
        categories,
    })
}
