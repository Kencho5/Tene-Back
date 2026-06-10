use sqlx::PgPool;

use crate::{
    error::Result,
    models::{
        AnalyticsPeriod, AnalyticsQuery, AnalyticsResponse, Brand, CableType, CableTypeRequest,
        CableVariant, CableVariantRequest, CableVariantUpdate, CartSnapshotItem, CheckoutEventRow,
        CheckoutSessionQuery, CheckoutSessionSummary, CheckoutSessionsResponse, ConversionRate,
        HighViewsLowSales, MostViewedProduct, Order,
        OrderQuery, OrderSearchResponse, Product, ProductImage, ProductRequest, ProductSeo,
        ProductSeoRequest, TrendingProduct,
        UniqueViewersProduct, UserQuery, UserRequest, UserResponse, UserSearchResponse,
        ViewsByHour,
    },
};

pub async fn create_product(pool: &PgPool, req: &ProductRequest) -> Result<Product> {
    let product = sqlx::query_as::<_, Product>(
        r#"
        INSERT INTO products (
            id, name, description, price, discount, quantity,
            specifications, brand_id, cable_type_id, warranty, enabled
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING *, (SELECT name FROM brands WHERE id = brand_id) as brand_name
        "#,
    )
    .bind(&req.id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.price)
    .bind(req.discount.unwrap_or_else(|| rust_decimal::Decimal::ZERO))
    .bind(req.quantity.unwrap_or(0))
    .bind(
        req.specifications
            .as_ref()
            .unwrap_or(&serde_json::json!({})),
    )
    .bind(&req.brand_id)
    .bind(&req.cable_type_id)
    .bind(&req.warranty)
    .bind(req.enabled.unwrap_or(true))
    .fetch_one(pool)
    .await?;

    Ok(product)
}

pub async fn update_product(pool: &PgPool, id: &str, req: &ProductRequest) -> Result<Product> {
    let product = sqlx::query_as::<_, Product>(
        r#"
        UPDATE products
        SET
            name = COALESCE($1, name),
            description = COALESCE($2, description),
            price = COALESCE($3, price),
            discount = COALESCE($4, discount),
            specifications = COALESCE($5, specifications),
            brand_id = COALESCE($6, brand_id),
            cable_type_id = COALESCE($7, cable_type_id),
            warranty = COALESCE($8, warranty),
            enabled = COALESCE($9, enabled),
            updated_at = NOW()
        WHERE id = $10
        RETURNING *, (SELECT name FROM brands WHERE id = brand_id) as brand_name
        "#,
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.price)
    .bind(&req.discount)
    .bind(&req.specifications)
    .bind(&req.brand_id)
    .bind(&req.cable_type_id)
    .bind(&req.warranty)
    .bind(&req.enabled)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(product)
}

// brands
pub async fn get_brands(pool: &PgPool) -> Result<Vec<Brand>> {
    let brands = sqlx::query_as::<_, Brand>("SELECT * FROM brands ORDER BY name ASC")
        .fetch_all(pool)
        .await?;
    Ok(brands)
}

pub async fn create_brand(pool: &PgPool, name: &str) -> Result<Brand> {
    let brand = sqlx::query_as::<_, Brand>(
        "INSERT INTO brands (name) VALUES ($1) RETURNING *",
    )
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(brand)
}

pub async fn update_brand(pool: &PgPool, id: i32, name: &str) -> Result<Brand> {
    let brand = sqlx::query_as::<_, Brand>(
        "UPDATE brands SET name = $1 WHERE id = $2 RETURNING *",
    )
    .bind(name)
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(brand)
}

pub async fn delete_brand(pool: &PgPool, id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM brands WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn find_brand_by_name(pool: &PgPool, name: &str) -> Result<Option<Brand>> {
    let brand = sqlx::query_as::<_, Brand>("SELECT * FROM brands WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    Ok(brand)
}

pub async fn find_brand_by_id(pool: &PgPool, id: i32) -> Result<Option<Brand>> {
    let brand = sqlx::query_as::<_, Brand>("SELECT * FROM brands WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(brand)
}

pub async fn delete_product(pool: &PgPool, id: &str) -> Result<u64> {
    let result = sqlx::query("DELETE FROM products WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

pub async fn add_product_image(
    pool: &PgPool,
    product_id: &str,
    image_uuid: uuid::Uuid,
    color: Option<String>,
    is_primary: bool,
    extension: &str,
    quantity: i32,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO product_images(product_id, image_uuid, color, is_primary, extension, quantity)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(product_id)
    .bind(image_uuid)
    .bind(color)
    .bind(is_primary)
    .bind(extension)
    .bind(quantity)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_product_image(
    pool: &PgPool,
    product_id: &str,
    image_uuid: uuid::Uuid,
) -> Result<Option<ProductImage>> {
    let deleted_image = sqlx::query_as::<_, ProductImage>(
        "DELETE FROM product_images WHERE product_id = $1 AND image_uuid = $2 RETURNING *",
    )
    .bind(product_id)
    .bind(image_uuid)
    .fetch_optional(pool)
    .await?;

    Ok(deleted_image)
}

pub async fn update_product_image_metadata(
    pool: &PgPool,
    product_id: &str,
    image_uuid: uuid::Uuid,
    color: Option<String>,
    is_primary: Option<bool>,
    quantity: Option<i32>,
) -> Result<Option<ProductImage>> {
    let updated_image = sqlx::query_as::<_, ProductImage>(
        r#"
        UPDATE product_images
        SET
            color = COALESCE($3, color),
            is_primary = COALESCE($4, is_primary),
            quantity = COALESCE($5, quantity)
        WHERE product_id = $1 AND image_uuid = $2
        RETURNING *
        "#,
    )
    .bind(product_id)
    .bind(image_uuid)
    .bind(color)
    .bind(is_primary)
    .bind(quantity)
    .fetch_optional(pool)
    .await?;

    Ok(updated_image)
}

const DEFAULT_PAGE_SIZE: i64 = 6;
const MAX_PAGE_SIZE: i64 = 100;

pub async fn search_users(pool: &PgPool, params: UserQuery) -> Result<UserSearchResponse> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0);

    let mut query_builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT id, email, name, role, created_at, COUNT(*) OVER() as total_count FROM users WHERE 1=1",
    );

    if let Some(id) = params.id {
        query_builder.push(" AND id = ");
        query_builder.push_bind(id);
    }

    if let Some(ref email) = params.email {
        query_builder.push(" AND email ILIKE ");
        query_builder.push_bind(format!("%{}%", email));
    }

    query_builder.push(" ORDER BY created_at DESC");

    query_builder.push(" LIMIT ");
    query_builder.push_bind(limit);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset);

    #[derive(sqlx::FromRow)]
    struct SearchResult {
        #[sqlx(flatten)]
        user: UserResponse,
        total_count: i64,
    }

    let results = query_builder
        .build_query_as::<SearchResult>()
        .fetch_all(pool)
        .await?;

    let total = results.first().map(|r| r.total_count).unwrap_or(0);
    let users = results.into_iter().map(|r| r.user).collect();

    Ok(UserSearchResponse {
        users,
        total,
        limit,
        offset,
    })
}

pub async fn update_user(pool: &PgPool, id: i32, req: &UserRequest) -> Result<UserResponse> {
    let user = sqlx::query_as::<_, UserResponse>(
        r#"
        UPDATE users
        SET
            email = COALESCE($1, email),
            name = COALESCE($2, name),
            role = COALESCE($3, role),
            updated_at = NOW()
        WHERE id = $4
        RETURNING id, email, name, role, created_at
        "#,
    )
    .bind(&req.email)
    .bind(&req.name)
    .bind(&req.role)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

pub async fn get_operator_emails(pool: &PgPool) -> Result<Vec<String>> {
    let emails = sqlx::query_scalar::<_, String>(
        "SELECT email FROM users WHERE role = 'operator'",
    )
    .fetch_all(pool)
    .await?;

    Ok(emails)
}

pub async fn delete_user(pool: &PgPool, id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

pub async fn get_orders(pool: &PgPool, params: OrderQuery) -> Result<OrderSearchResponse> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0);

    let mut query_builder = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT *, COUNT(*) OVER() as total_count FROM orders WHERE 1=1",
    );

    if let Some(id) = params.id {
        query_builder.push(" AND id = ");
        query_builder.push_bind(id);
    }

    if let Some(search) = params.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        query_builder.push(" AND (");
        if let Ok(search_id) = search.parse::<i32>() {
            query_builder.push("id = ");
            query_builder.push_bind(search_id);
            query_builder.push(" OR ");
        }
        let pattern = format!("%{}%", search);
        query_builder.push("customer_name ILIKE ");
        query_builder.push_bind(pattern.clone());
        query_builder.push(" OR customer_surname ILIKE ");
        query_builder.push_bind(pattern);
        query_builder.push(")");
    }

    if let Some(user_id) = params.user_id {
        query_builder.push(" AND user_id = ");
        query_builder.push_bind(user_id);
    }

    if let Some(ref status) = params.status {
        query_builder.push(" AND status = ");
        query_builder.push_bind(status);
    }

    query_builder.push(" ORDER BY created_at DESC");
    query_builder.push(" LIMIT ");
    query_builder.push_bind(limit);
    query_builder.push(" OFFSET ");
    query_builder.push_bind(offset);

    #[derive(sqlx::FromRow)]
    struct SearchResult {
        #[sqlx(flatten)]
        order: Order,
        total_count: i64,
    }

    let results = query_builder
        .build_query_as::<SearchResult>()
        .fetch_all(pool)
        .await?;

    let total = results.first().map(|r| r.total_count).unwrap_or(0);
    let orders: Vec<Order> = results.into_iter().map(|r| r.order).collect();

    let order_db_ids: Vec<i32> = orders.iter().map(|o| o.id).collect();
    let all_items =
        crate::queries::order_queries::get_items_for_orders(pool, &order_db_ids).await?;

    let mut items_map: std::collections::HashMap<i32, Vec<_>> = std::collections::HashMap::new();
    for item in all_items {
        items_map.entry(item.order_id).or_default().push(item);
    }

    let orders = orders
        .into_iter()
        .map(|order| {
            let items = items_map.remove(&order.id).unwrap_or_default();
            crate::models::OrderResponse {
                order,
                items,
                comment_images: Vec::new(),
            }
        })
        .collect();

    Ok(OrderSearchResponse {
        orders,
        total,
        limit,
        offset,
    })
}

pub async fn get_analytics(pool: &PgPool, params: AnalyticsQuery) -> Result<AnalyticsResponse> {
    // these fragments come from a fixed enum so no injection risk
    let (where_pv, where_bare, join_pv) = match params.period {
        Some(AnalyticsPeriod::Today) => (
            "WHERE pv.viewed_at >= CURRENT_DATE",
            "WHERE viewed_at >= CURRENT_DATE",
            "AND pv.viewed_at >= CURRENT_DATE",
        ),
        Some(AnalyticsPeriod::Yesterday) => (
            "WHERE pv.viewed_at >= CURRENT_DATE - INTERVAL '1 day' AND pv.viewed_at < CURRENT_DATE",
            "WHERE viewed_at >= CURRENT_DATE - INTERVAL '1 day' AND viewed_at < CURRENT_DATE",
            "AND pv.viewed_at >= CURRENT_DATE - INTERVAL '1 day' AND pv.viewed_at < CURRENT_DATE",
        ),
        Some(AnalyticsPeriod::Last7Days) => (
            "WHERE pv.viewed_at >= CURRENT_DATE - INTERVAL '7 days'",
            "WHERE viewed_at >= CURRENT_DATE - INTERVAL '7 days'",
            "AND pv.viewed_at >= CURRENT_DATE - INTERVAL '7 days'",
        ),
        Some(AnalyticsPeriod::Last30Days) => (
            "WHERE pv.viewed_at >= CURRENT_DATE - INTERVAL '30 days'",
            "WHERE viewed_at >= CURRENT_DATE - INTERVAL '30 days'",
            "AND pv.viewed_at >= CURRENT_DATE - INTERVAL '30 days'",
        ),
        None => ("", "", ""),
    };

    let most_viewed = sqlx::query_as::<_, MostViewedProduct>(
        &format!(
            "SELECT pv.product_id, p.name as product_name, COUNT(*) as views
             FROM product_views pv
             JOIN products p ON p.id = pv.product_id
             {where_pv}
             GROUP BY pv.product_id, p.name
             ORDER BY views DESC
             LIMIT 10"
        ),
    )
    .fetch_all(pool)
    .await?;

    let trending_this_week = sqlx::query_as::<_, TrendingProduct>(
        &format!(
            "SELECT pv.product_id, p.name as product_name, COUNT(*) as views
             FROM product_views pv
             JOIN products p ON p.id = pv.product_id
             WHERE pv.viewed_at >= CURRENT_DATE - INTERVAL '7 days'
             GROUP BY pv.product_id, p.name
             ORDER BY views DESC
             LIMIT 10"
        ),
    )
    .fetch_all(pool)
    .await?;

    let unique_viewers = sqlx::query_as::<_, UniqueViewersProduct>(
        &format!(
            "SELECT pv.product_id, p.name as product_name,
                    COUNT(DISTINCT pv.user_id) as logged_in_viewers,
                    COUNT(*) FILTER (WHERE pv.user_id IS NULL) as anonymous_views,
                    COUNT(*) as total_views
             FROM product_views pv
             JOIN products p ON p.id = pv.product_id
             {where_pv}
             GROUP BY pv.product_id, p.name
             ORDER BY total_views DESC
             LIMIT 10"
        ),
    )
    .fetch_all(pool)
    .await?;

    let views_by_hour = sqlx::query_as::<_, ViewsByHour>(
        &format!(
            "SELECT EXTRACT(HOUR FROM viewed_at) as hour, COUNT(*) as views
             FROM product_views
             {where_bare}
             GROUP BY hour
             ORDER BY hour"
        ),
    )
    .fetch_all(pool)
    .await?;

    let high_views_low_sales = sqlx::query_as::<_, HighViewsLowSales>(
        &format!(
            "SELECT p.id as product_id, p.name as product_name,
                    COUNT(pv.id) as views,
                    COALESCE(SUM(oi.quantity), 0) as sold
             FROM products p
             LEFT JOIN product_views pv ON pv.product_id = p.id {join_pv}
             LEFT JOIN order_items oi ON oi.product_id = p.id
             GROUP BY p.id, p.name
             HAVING COUNT(pv.id) > 0 AND COALESCE(SUM(oi.quantity), 0) = 0
             ORDER BY views DESC
             LIMIT 10"
        ),
    )
    .fetch_all(pool)
    .await?;

    let conversion_rates = sqlx::query_as::<_, ConversionRate>(
        &format!(
            "SELECT p.id as product_id, p.name as product_name,
                    COUNT(DISTINCT pv.user_id) as viewers,
                    COUNT(DISTINCT oi.order_id) as purchases,
                    ROUND(
                        COUNT(DISTINCT oi.order_id)::numeric /
                        NULLIF(COUNT(DISTINCT pv.user_id), 0) * 100, 2
                    ) as conversion_pct
             FROM products p
             LEFT JOIN product_views pv ON pv.product_id = p.id {join_pv}
             LEFT JOIN order_items oi ON oi.product_id = p.id
             GROUP BY p.id, p.name
             HAVING COUNT(DISTINCT pv.user_id) > 0
             ORDER BY conversion_pct DESC
             LIMIT 10"
        ),
    )
    .fetch_all(pool)
    .await?;

    Ok(AnalyticsResponse {
        most_viewed,
        trending_this_week,
        unique_viewers,
        views_by_hour,
        high_views_low_sales,
        conversion_rates,
    })
}

// cable types
pub async fn get_cable_types(pool: &PgPool) -> Result<Vec<CableType>> {
    let types = sqlx::query_as::<_, CableType>("SELECT * FROM cable_types ORDER BY name ASC")
        .fetch_all(pool)
        .await?;
    Ok(types)
}

pub async fn find_cable_type_by_id(pool: &PgPool, id: i32) -> Result<Option<CableType>> {
    let t = sqlx::query_as::<_, CableType>("SELECT * FROM cable_types WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(t)
}

pub async fn find_cable_type_by_name(pool: &PgPool, name: &str) -> Result<Option<CableType>> {
    let t = sqlx::query_as::<_, CableType>("SELECT * FROM cable_types WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    Ok(t)
}

pub async fn create_cable_type(pool: &PgPool, req: &CableTypeRequest) -> Result<CableType> {
    let t = sqlx::query_as::<_, CableType>(
        "INSERT INTO cable_types (name) VALUES ($1) RETURNING *",
    )
    .bind(&req.name)
    .fetch_one(pool)
    .await?;
    Ok(t)
}

pub async fn update_cable_type(
    pool: &PgPool,
    id: i32,
    req: &CableTypeRequest,
) -> Result<CableType> {
    let t = sqlx::query_as::<_, CableType>(
        "UPDATE cable_types SET name = $1, updated_at = NOW() WHERE id = $2 RETURNING *",
    )
    .bind(&req.name)
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(t)
}

pub async fn delete_cable_type(pool: &PgPool, id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM cable_types WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

// cable variants
pub async fn get_cable_variants_by_type(
    pool: &PgPool,
    cable_type_id: i32,
) -> Result<Vec<CableVariant>> {
    let variants = sqlx::query_as::<_, CableVariant>(
        "SELECT * FROM cable_variants WHERE cable_type_id = $1 ORDER BY watts ASC, length_cm ASC",
    )
    .bind(cable_type_id)
    .fetch_all(pool)
    .await?;
    Ok(variants)
}

pub async fn find_cable_variant_by_id(
    pool: &PgPool,
    id: i32,
) -> Result<Option<CableVariant>> {
    let v = sqlx::query_as::<_, CableVariant>("SELECT * FROM cable_variants WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(v)
}

pub async fn find_cable_variant_by_combo(
    pool: &PgPool,
    cable_type_id: i32,
    watts: i32,
    length_cm: i32,
) -> Result<Option<CableVariant>> {
    let v = sqlx::query_as::<_, CableVariant>(
        "SELECT * FROM cable_variants WHERE cable_type_id = $1 AND watts = $2 AND length_cm = $3",
    )
    .bind(cable_type_id)
    .bind(watts)
    .bind(length_cm)
    .fetch_optional(pool)
    .await?;
    Ok(v)
}

pub async fn create_cable_variant(
    pool: &PgPool,
    cable_type_id: i32,
    req: &CableVariantRequest,
) -> Result<CableVariant> {
    let v = sqlx::query_as::<_, CableVariant>(
        r#"
        INSERT INTO cable_variants (cable_type_id, watts, length_cm, price, warranty_months)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(cable_type_id)
    .bind(req.watts)
    .bind(req.length_cm)
    .bind(req.price)
    .bind(req.warranty_months)
    .fetch_one(pool)
    .await?;
    Ok(v)
}

pub async fn update_cable_variant(
    pool: &PgPool,
    id: i32,
    req: &CableVariantUpdate,
) -> Result<CableVariant> {
    let v = sqlx::query_as::<_, CableVariant>(
        r#"
        UPDATE cable_variants
        SET watts = COALESCE($1, watts),
            length_cm = COALESCE($2, length_cm),
            price = COALESCE($3, price),
            warranty_months = COALESCE($4, warranty_months),
            updated_at = NOW()
        WHERE id = $5
        RETURNING *
        "#,
    )
    .bind(req.watts)
    .bind(req.length_cm)
    .bind(req.price)
    .bind(req.warranty_months)
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(v)
}

pub async fn delete_cable_variant(pool: &PgPool, id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM cable_variants WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn replace_top_products(pool: &PgPool, product_ids: &[String]) -> Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM top_products")
        .execute(&mut *tx)
        .await?;

    if !product_ids.is_empty() {
        let positions: Vec<i32> = (0..product_ids.len() as i32).collect();
        sqlx::query(
            "INSERT INTO top_products (product_id, position)
             SELECT * FROM UNNEST($1::text[], $2::int[])",
        )
        .bind(product_ids)
        .bind(&positions)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn get_product_seo(pool: &PgPool, product_id: &str) -> Result<Option<ProductSeo>> {
    let seo = sqlx::query_as::<_, ProductSeo>(
        "SELECT meta_title, meta_description, meta_keywords, slug, search_terms, faqs,
                og_image_uuid, no_index
         FROM product_seo WHERE product_id = $1",
    )
    .bind(product_id)
    .fetch_optional(pool)
    .await?;
    Ok(seo)
}

pub async fn find_product_seo_by_slug(
    pool: &PgPool,
    slug: &str,
) -> Result<Option<String>> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT product_id FROM product_seo WHERE slug = $1")
            .bind(slug)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(id,)| id))
}

pub async fn upsert_product_seo(
    pool: &PgPool,
    product_id: &str,
    req: &ProductSeoRequest,
) -> Result<ProductSeo> {
    let faqs_json = serde_json::to_value(&req.faqs)?;
    let seo = sqlx::query_as::<_, ProductSeo>(
        r#"
        INSERT INTO product_seo (
            product_id, meta_title, meta_description, meta_keywords, slug,
            search_terms, faqs, og_image_uuid, no_index, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
        ON CONFLICT (product_id) DO UPDATE SET
            meta_title       = EXCLUDED.meta_title,
            meta_description = EXCLUDED.meta_description,
            meta_keywords    = EXCLUDED.meta_keywords,
            slug             = EXCLUDED.slug,
            search_terms     = EXCLUDED.search_terms,
            faqs             = EXCLUDED.faqs,
            og_image_uuid    = EXCLUDED.og_image_uuid,
            no_index         = EXCLUDED.no_index,
            updated_at       = NOW()
        RETURNING meta_title, meta_description, meta_keywords, slug, search_terms,
                  faqs, og_image_uuid, no_index
        "#,
    )
    .bind(product_id)
    .bind(&req.meta_title)
    .bind(&req.meta_description)
    .bind(&req.meta_keywords)
    .bind(&req.slug)
    .bind(&req.search_terms)
    .bind(&faqs_json)
    .bind(&req.og_image_uuid)
    .bind(req.no_index)
    .fetch_one(pool)
    .await?;
    Ok(seo)
}

pub async fn get_top_product_ids(pool: &PgPool, limit: Option<i64>) -> Result<Vec<String>> {
    let mut q = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT product_id FROM top_products ORDER BY position ASC",
    );
    if let Some(l) = limit {
        q.push(" LIMIT ");
        q.push_bind(l);
    }
    let rows: Vec<(String,)> = q.build_query_as().fetch_all(pool).await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

pub async fn get_checkout_sessions(
    pool: &PgPool,
    params: CheckoutSessionQuery,
) -> Result<CheckoutSessionsResponse> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0);

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "WITH sessions AS (
            SELECT
                session_id,
                MAX(user_id) AS user_id,
                BOOL_OR(COALESCE(is_guest, false)) AS is_guest,
                BOOL_OR(type = 'purchase') AS purchased,
                MAX(order_id) AS order_id,
                MAX(step_index) AS last_step_index,
                COUNT(*) AS event_count,
                MIN(created_at) AS started_at,
                MAX(created_at) AS last_activity_at,
                COUNT(*) OVER() AS total_count
            FROM checkout_analytics
            GROUP BY session_id
        )
        SELECT * FROM sessions WHERE 1=1",
    );

    if let Some(session_id) = params.session_id {
        qb.push(" AND session_id = ");
        qb.push_bind(session_id);
    }
    if let Some(user_id) = params.user_id {
        qb.push(" AND user_id = ");
        qb.push_bind(user_id);
    }
    if let Some(ref step) = params.step {
        qb.push(" AND session_id IN (SELECT session_id FROM checkout_analytics WHERE step = ");
        qb.push_bind(step);
        qb.push(")");
    }
    match params.outcome.as_deref() {
        Some("completed") => {
            qb.push(" AND purchased = true");
        }
        Some("abandoned") => {
            qb.push(" AND purchased = false");
        }
        _ => {}
    }

    qb.push(" ORDER BY last_activity_at DESC LIMIT ");
    qb.push_bind(limit);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    #[derive(sqlx::FromRow)]
    struct SessionRow {
        session_id: uuid::Uuid,
        user_id: Option<i32>,
        is_guest: Option<bool>,
        purchased: Option<bool>,
        order_id: Option<String>,
        last_step_index: Option<i32>,
        event_count: i64,
        started_at: chrono::DateTime<chrono::Utc>,
        last_activity_at: chrono::DateTime<chrono::Utc>,
        total_count: i64,
    }

    let rows = qb.build_query_as::<SessionRow>().fetch_all(pool).await?;
    let total = rows.first().map(|r| r.total_count).unwrap_or(0);

    let session_ids: Vec<uuid::Uuid> = rows.iter().map(|r| r.session_id).collect();

    let events = sqlx::query_as::<_, CheckoutEventRow>(
        "SELECT id, session_id, type, step, step_index, field, value, order_id,
                is_guest, user_id, client_timestamp, created_at
         FROM checkout_analytics
         WHERE session_id = ANY($1)
         ORDER BY created_at ASC",
    )
    .bind(&session_ids)
    .fetch_all(pool)
    .await?;

    let order_ids: Vec<String> = rows.iter().filter_map(|r| r.order_id.clone()).collect();
    let order_statuses: Vec<(String, String)> = sqlx::query_as(
        "SELECT order_id, status FROM orders WHERE order_id = ANY($1)",
    )
    .bind(&order_ids)
    .fetch_all(pool)
    .await?;
    let status_map: std::collections::HashMap<String, String> =
        order_statuses.into_iter().collect();

    let carts: Vec<(uuid::Uuid, serde_json::Value)> = sqlx::query_as(
        "SELECT session_id, cart FROM checkout_cart_snapshots WHERE session_id = ANY($1)",
    )
    .bind(&session_ids)
    .fetch_all(pool)
    .await?;
    let mut cart_map: std::collections::HashMap<uuid::Uuid, Vec<CartSnapshotItem>> =
        std::collections::HashMap::new();
    for (sid, cart_json) in carts {
        if let Ok(items) = serde_json::from_value::<Vec<CartSnapshotItem>>(cart_json) {
            cart_map.insert(sid, items);
        }
    }

    let mut events_map: std::collections::HashMap<uuid::Uuid, Vec<CheckoutEventRow>> =
        std::collections::HashMap::new();
    for event in events {
        events_map.entry(event.session_id).or_default().push(event);
    }

    let sessions = rows
        .into_iter()
        .map(|r| {
            let session_events = events_map.remove(&r.session_id).unwrap_or_default();

            let mut fields = std::collections::HashMap::new();
            let mut last_step = None;
            let mut last_step_index = r.last_step_index;
            for event in &session_events {
                if let (Some(field), Some(value)) = (&event.field, &event.value) {
                    fields.insert(field.clone(), value.clone());
                }
                if event.step_index >= last_step_index {
                    last_step_index = event.step_index;
                    last_step = event.step.clone();
                }
            }

            let order_status = r
                .order_id
                .as_ref()
                .and_then(|id| status_map.get(id).cloned());

            CheckoutSessionSummary {
                session_id: r.session_id,
                user_id: r.user_id,
                is_guest: r.is_guest,
                last_step,
                last_step_index,
                purchased: r.purchased.unwrap_or(false),
                order_id: r.order_id,
                order_status,
                event_count: r.event_count,
                started_at: r.started_at,
                last_activity_at: r.last_activity_at,
                fields,
                cart: cart_map.remove(&r.session_id),
                events: session_events,
            }
        })
        .collect();

    Ok(CheckoutSessionsResponse {
        sessions,
        total,
        limit,
        offset,
    })
}
