use std::collections::HashMap;

use sqlx::PgPool;

use crate::{
    error::Result,
    models::{
        BrandFacetValue, CableVariant, Category, CategoryFacetValue, FacetValue, Product,
        ProductFacets, ProductImage, ProductQuery, ProductResponse, SaleType, SortBy,
    },
};

pub async fn find_cable_variants_by_type_ids(
    pool: &PgPool,
    type_ids: &[i32],
) -> Result<HashMap<(i32, i32, i32), CableVariant>> {
    if type_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let variants = sqlx::query_as::<_, CableVariant>(
        "SELECT * FROM cable_variants WHERE cable_type_id = ANY($1)",
    )
    .bind(type_ids)
    .fetch_all(pool)
    .await?;

    Ok(variants
        .into_iter()
        .map(|v| ((v.cable_type_id, v.watts, v.length_cm), v))
        .collect())
}

pub async fn find_product_bundle(
    pool: &PgPool,
    id: &str,
) -> Result<Option<(Product, Vec<ProductImage>, Vec<Category>, Option<crate::models::ProductSeo>)>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        product: Product,
        images_json: serde_json::Value,
        categories_json: serde_json::Value,
        seo_json: Option<serde_json::Value>,
    }

    let row = sqlx::query_as::<_, Row>(
        r#"
        SELECT
            p.*,
            b.name AS brand_name,
            COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'product_id', pi.product_id,
                        'image_uuid', pi.image_uuid,
                        'color', pi.color,
                        'is_primary', pi.is_primary,
                        'extension', pi.extension,
                        'quantity', pi.quantity
                    )
                    ORDER BY pi.is_primary DESC, pi.created_at ASC
                )
                FROM product_images pi
                WHERE pi.product_id = p.id
            ), '[]'::jsonb) AS images_json,
            COALESCE((
                SELECT jsonb_agg(to_jsonb(c) ORDER BY c.display_order ASC, c.name ASC)
                FROM product_categories pc
                JOIN categories c ON c.id = pc.category_id
                WHERE pc.product_id = p.id
            ), '[]'::jsonb) AS categories_json,
            (
                SELECT jsonb_build_object(
                    'meta_title', s.meta_title,
                    'meta_description', s.meta_description,
                    'meta_keywords', s.meta_keywords,
                    'slug', s.slug,
                    'search_terms', s.search_terms,
                    'faqs', s.faqs,
                    'og_image_uuid', s.og_image_uuid,
                    'no_index', s.no_index
                )
                FROM product_seo s
                WHERE s.product_id = p.id
            ) AS seo_json
        FROM products p
        LEFT JOIN brands b ON p.brand_id = b.id
        WHERE p.id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else { return Ok(None) };

    let images: Vec<ProductImage> = serde_json::from_value(row.images_json)?;
    let categories: Vec<Category> = serde_json::from_value(row.categories_json)?;
    let seo: Option<crate::models::ProductSeo> = match row.seo_json {
        Some(v) => Some(serde_json::from_value(v)?),
        None => None,
    };

    Ok(Some((row.product, images, categories, seo)))
}

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

pub async fn build_products_response_ordered(
    pool: &PgPool,
    ordered_ids: &[String],
) -> Result<Vec<ProductResponse>> {
    if ordered_ids.is_empty() {
        return Ok(Vec::new());
    }

    let products_map = find_by_ids(pool, ordered_ids).await?;
    let mut image_groups = find_images_by_product_ids(pool, ordered_ids).await?;

    let mut out = Vec::with_capacity(ordered_ids.len());
    for id in ordered_ids {
        if let Some(product) = products_map.get(id).cloned() {
            let images = image_groups.remove(id).unwrap_or_default();
            out.push(ProductResponse {
                data: product,
                images,
                categories: Vec::new(),
                seo: None,
            });
        }
    }
    Ok(out)
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

const DEFAULT_PAGE_SIZE: i64 = 12;
const MAX_PAGE_SIZE: i64 = 100;

pub async fn search_products(
    pool: &PgPool,
    params: ProductQuery,
) -> Result<crate::models::ProductSearchResponse> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0);

    if let Some(ref id) = params.id {
        let bundle = find_product_bundle(pool, id).await?;
        let matched = bundle.filter(|(p, _, _, _)| match params.enabled {
            Some(enabled) => p.enabled == enabled,
            None => true,
        });

        return match matched {
            Some((product, images, categories, seo)) => {
                Ok(crate::models::ProductSearchResponse {
                    products: vec![ProductResponse {
                        data: product,
                        images,
                        categories,
                        seo,
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

    let needs_views = matches!(params.sort_by, Some(SortBy::ViewsDesc));
    let has_query = params.query.is_some();

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new("SELECT p.*, b.name as brand_name");

    if has_query {
        let q = params.query.as_ref().unwrap();
        qb.push(", GREATEST(similarity(p.name, ");
        qb.push_bind(q);
        qb.push("), similarity(COALESCE(p.description, ''), ");
        qb.push_bind(q);
        qb.push(")) as relevance_score");
    }
    if needs_views {
        qb.push(", COALESCE(pvc.view_count, 0) as view_count");
    }
    qb.push(", COUNT(*) OVER() as total_count FROM products p LEFT JOIN brands b ON p.brand_id = b.id");
    if needs_views {
        qb.push(
            " LEFT JOIN (
                SELECT product_id, COUNT(*) AS view_count
                FROM product_views
                WHERE viewed_at >= NOW() - INTERVAL '7 days'
                GROUP BY product_id
            ) pvc ON pvc.product_id = p.id",
        );
    }
    qb.push(" WHERE 1=1");

    if let Some(enabled) = params.enabled {
        qb.push(" AND p.enabled = ");
        qb.push_bind(enabled);
    }

    if params.in_stock == Some(true) {
        qb.push(" AND p.quantity > 0");
    }

    if let Some(q) = &params.query {
        let like_q = format!("%{}%", escape_like(q));
        qb.push(" AND (p.name ILIKE ");
        qb.push_bind(like_q.clone());
        qb.push(" OR COALESCE(p.description, '') ILIKE ");
        qb.push_bind(like_q);
        qb.push(" OR p.name % ");
        qb.push_bind(q);
        qb.push(" OR COALESCE(p.description, '') % ");
        qb.push_bind(q);
        qb.push(")");
    }

    if let Some(min_price) = params.price_from {
        qb.push(" AND p.price >= ");
        qb.push_bind(min_price);
    }
    if let Some(max_price) = params.price_to {
        qb.push(" AND p.price <= ");
        qb.push_bind(max_price);
    }

    if let Some(brand_id) = params.brand {
        qb.push(" AND p.brand_id = ");
        qb.push_bind(brand_id);
    }

    if !params.color.is_empty() {
        qb.push(" AND EXISTS (SELECT 1 FROM product_images pi WHERE pi.product_id = p.id AND pi.color = ANY(");
        qb.push_bind(&params.color);
        qb.push("))");
    }

    let all_category_ids: Vec<i32> = params
        .parent_category_id
        .iter()
        .chain(params.child_category_id.iter())
        .copied()
        .collect();

    if !all_category_ids.is_empty() {
        qb.push(
            " AND EXISTS (
                SELECT 1 FROM product_categories pc
                WHERE pc.product_id = p.id
                AND pc.category_id IN (
                    WITH RECURSIVE ct(id) AS (
                        SELECT id FROM categories WHERE id = ANY(",
        );
        qb.push_bind(&all_category_ids);
        qb.push(
            ")
                        UNION ALL
                        SELECT c.id FROM categories c JOIN ct ON c.parent_id = ct.id
                    )
                    SELECT id FROM ct
                )
            )",
        );
    }

    let has_discount = params.sale_type.contains(&SaleType::Discount);
    let has_coins = params.sale_type.contains(&SaleType::Coins);

    if has_discount && !has_coins {
        qb.push(" AND p.discount > 0");
    } else if !has_discount && has_coins {
        qb.push(" AND false");
    }

    match params.sort_by {
        Some(SortBy::PriceAsc) => {
            qb.push(" ORDER BY p.price ASC");
            if has_query {
                qb.push(", relevance_score DESC");
            }
            qb.push(", p.created_at DESC");
        }
        Some(SortBy::PriceDesc) => {
            qb.push(" ORDER BY p.price DESC");
            if has_query {
                qb.push(", relevance_score DESC");
            }
            qb.push(", p.created_at DESC");
        }
        Some(SortBy::ViewsDesc) => {
            qb.push(" ORDER BY view_count DESC");
            if has_query {
                qb.push(", relevance_score DESC");
            }
            qb.push(", p.created_at DESC");
        }
        None => {
            if has_query {
                qb.push(" ORDER BY relevance_score DESC, p.created_at DESC");
            } else {
                qb.push(" ORDER BY p.created_at DESC");
            }
        }
    };

    qb.push(" LIMIT ");
    qb.push_bind(limit);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    #[derive(sqlx::FromRow)]
    struct SearchResult {
        #[sqlx(flatten)]
        product: Product,
        total_count: i64,
    }

    let results = qb
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

    let product_ids: Vec<String> = results.iter().map(|r| r.product.id.clone()).collect();

    #[derive(sqlx::FromRow)]
    struct ProductCategoryRow {
        product_id: String,
        #[sqlx(flatten)]
        category: Category,
    }

    let images_fut = sqlx::query_as::<_, ProductImage>(
        "SELECT product_id, image_uuid, color, is_primary, extension, quantity
         FROM product_images
         WHERE product_id = ANY($1)
         ORDER BY product_id, is_primary DESC, created_at ASC",
    )
    .bind(&product_ids)
    .fetch_all(pool);

    let categories_fut = sqlx::query_as::<_, ProductCategoryRow>(
        "SELECT pc.product_id, c.*
         FROM product_categories pc
         INNER JOIN categories c ON pc.category_id = c.id
         WHERE pc.product_id = ANY($1)
         ORDER BY pc.product_id, c.display_order ASC, c.name ASC",
    )
    .bind(&product_ids)
    .fetch_all(pool);

    let (images, categories) = tokio::try_join!(images_fut, categories_fut)?;

    let mut image_groups: HashMap<String, Vec<ProductImage>> =
        images
            .into_iter()
            .fold(HashMap::with_capacity(product_ids.len()), |mut acc, img| {
                acc.entry(img.product_id.clone()).or_default().push(img);
                acc
            });

    let mut category_groups: HashMap<String, Vec<Category>> =
        categories
            .into_iter()
            .fold(HashMap::with_capacity(product_ids.len()), |mut acc, row| {
                acc.entry(row.product_id.clone())
                    .or_default()
                    .push(row.category);
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
            seo: None,
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
        "WITH src_cats AS (
             SELECT category_id FROM product_categories WHERE product_id = $1
         ),
         scored AS (
             SELECT pc.product_id, COUNT(*) AS shared
             FROM product_categories pc
             JOIN src_cats sc ON sc.category_id = pc.category_id
             WHERE pc.product_id <> $1
             GROUP BY pc.product_id
         )
         SELECT p.*, b.name as brand_name
         FROM scored s
         JOIN products p ON p.id = s.product_id AND p.enabled = true
         LEFT JOIN brands b ON p.brand_id = b.id
         ORDER BY s.shared DESC, p.created_at DESC
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
                seo: None,
            }
        })
        .collect();

    Ok(responses)
}

pub async fn get_product_facets(pool: &PgPool, params: ProductQuery) -> Result<ProductFacets> {
    let has_discount = params.sale_type.contains(&SaleType::Discount);
    let has_coins = params.sale_type.contains(&SaleType::Coins);
    let force_empty = !has_discount && has_coins;

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new("");

    qb.push("WITH parent_descendants AS (");
    if params.parent_category_id.is_empty() {
        qb.push("SELECT NULL::int AS id WHERE false");
    } else {
        qb.push(
            "WITH RECURSIVE ct(id) AS (
                SELECT id FROM categories WHERE id = ANY(",
        );
        qb.push_bind(&params.parent_category_id);
        qb.push(
            ")
                UNION ALL
                SELECT c.id FROM categories c JOIN ct ON c.parent_id = ct.id
            )
            SELECT id FROM ct",
        );
    }
    qb.push("), child_descendants AS (");
    if params.child_category_id.is_empty() {
        qb.push("SELECT NULL::int AS id WHERE false");
    } else {
        qb.push(
            "WITH RECURSIVE ct(id) AS (
                SELECT id FROM categories WHERE id = ANY(",
        );
        qb.push_bind(&params.child_category_id);
        qb.push(
            ")
                UNION ALL
                SELECT c.id FROM categories c JOIN ct ON c.parent_id = ct.id
            )
            SELECT id FROM ct",
        );
    }

    qb.push("), base_ids AS (SELECT p.id FROM products p WHERE 1=1");

    if force_empty {
        qb.push(" AND false");
    }
    if let Some(enabled) = params.enabled {
        qb.push(" AND p.enabled = ");
        qb.push_bind(enabled);
    }
    if let Some(q) = &params.query {
        let like_q = format!("%{}%", escape_like(q));
        qb.push(" AND (p.name ILIKE ");
        qb.push_bind(like_q.clone());
        qb.push(" OR p.description ILIKE ");
        qb.push_bind(like_q);
        qb.push(" OR similarity(p.name, ");
        qb.push_bind(q);
        qb.push(") > 0.3 OR similarity(COALESCE(p.description, ''), ");
        qb.push_bind(q);
        qb.push(") > 0.3)");
    }
    if let Some(min_price) = params.price_from {
        qb.push(" AND p.price >= ");
        qb.push_bind(min_price);
    }
    if let Some(max_price) = params.price_to {
        qb.push(" AND p.price <= ");
        qb.push_bind(max_price);
    }
    if !params.color.is_empty() {
        qb.push(" AND EXISTS (SELECT 1 FROM product_images pi WHERE pi.product_id = p.id AND pi.color = ANY(");
        qb.push_bind(&params.color);
        qb.push("))");
    }
    if has_discount && !has_coins {
        qb.push(" AND p.discount > 0");
    }
    if !params.parent_category_id.is_empty() {
        qb.push(
            " AND EXISTS (
                SELECT 1 FROM product_categories pc
                WHERE pc.product_id = p.id
                AND pc.category_id IN (SELECT id FROM parent_descendants)
            )",
        );
    }

    qb.push("), category_filtered_ids AS (SELECT p.id FROM base_ids p");
    if !params.child_category_id.is_empty() {
        qb.push(
            " WHERE EXISTS (
                SELECT 1 FROM product_categories pc
                WHERE pc.product_id = p.id
                AND pc.category_id IN (SELECT id FROM child_descendants)
            )",
        );
    }

    qb.push("), filtered_ids AS (SELECT c.id FROM category_filtered_ids c");
    if let Some(brand_id) = params.brand {
        qb.push(" JOIN products p ON p.id = c.id WHERE p.brand_id = ");
        qb.push_bind(brand_id);
    }

    qb.push(
        "), brand_facet AS (
            SELECT b.id::text AS k1, b.name AS k2, COUNT(*)::bigint AS cnt
            FROM category_filtered_ids c
            JOIN products p ON p.id = c.id
            JOIN brands b ON p.brand_id = b.id
            GROUP BY b.id, b.name
            ORDER BY cnt DESC
            LIMIT 50
        ), color_facet AS (
            SELECT pi.color AS k1, NULL::text AS k2, COUNT(DISTINCT p.id)::bigint AS cnt
            FROM filtered_ids f
            JOIN products p ON p.id = f.id
            JOIN product_images pi ON pi.product_id = p.id
            WHERE pi.color IS NOT NULL AND pi.color != ''
            GROUP BY pi.color
            ORDER BY cnt DESC
            LIMIT 50
        ), category_facet AS (
            SELECT c.id::text AS k1, c.name AS k2, COUNT(DISTINCT p.id)::bigint AS cnt,
                   c.parent_id
            FROM base_ids b
            JOIN products p ON p.id = b.id
            JOIN product_categories pc ON pc.product_id = p.id
            JOIN categories c ON c.id = pc.category_id
            WHERE c.enabled = true",
    );
    if let Some(brand_id) = params.brand {
        qb.push(" AND p.brand_id = ");
        qb.push_bind(brand_id);
    }
    if !params.parent_category_id.is_empty() {
        qb.push(" AND c.id IN (SELECT id FROM parent_descendants)");
    }
    qb.push(
        " GROUP BY c.id, c.parent_id, c.name
            ORDER BY cnt DESC
            LIMIT 100
        )
        SELECT 'b'::text AS tag, k1, k2, cnt, NULL::int AS parent_id FROM brand_facet
        UNION ALL
        SELECT 'c'::text AS tag, k1, k2, cnt, NULL::int AS parent_id FROM color_facet
        UNION ALL
        SELECT 'g'::text AS tag, k1, k2, cnt, parent_id FROM category_facet",
    );

    #[derive(sqlx::FromRow)]
    struct Row {
        tag: String,
        k1: Option<String>,
        k2: Option<String>,
        cnt: i64,
        parent_id: Option<i32>,
    }

    let rows: Vec<Row> = qb.build_query_as().fetch_all(pool).await?;

    let mut brands = Vec::new();
    let mut colors = Vec::new();
    let mut categories = Vec::new();

    for r in rows {
        match r.tag.as_str() {
            "b" => {
                if let (Some(id_str), Some(name)) = (r.k1, r.k2) {
                    if let Ok(id) = id_str.parse::<i32>() {
                        brands.push(BrandFacetValue {
                            id,
                            name,
                            count: r.cnt,
                        });
                    }
                }
            }
            "c" => {
                if let Some(value) = r.k1 {
                    colors.push(FacetValue {
                        value,
                        count: r.cnt,
                    });
                }
            }
            "g" => {
                if let (Some(id_str), Some(name)) = (r.k1, r.k2) {
                    if let Ok(id) = id_str.parse::<i32>() {
                        categories.push(CategoryFacetValue {
                            id,
                            parent_id: r.parent_id,
                            name,
                            count: r.cnt,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    Ok(ProductFacets {
        brands,
        colors,
        categories,
    })
}

pub async fn add_product_views(
    pool: &PgPool,
    product_id: &str,
    user_id: Option<i32>,
) -> Result<()> {
    sqlx::query("INSERT INTO product_views(product_id, user_id) VALUES($1, $2)")
        .bind(product_id)
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}
