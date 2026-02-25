use sqlx::PgPool;

use crate::{
    error::Result,
    models::{
        Order, OrderQuery, OrderSearchResponse, Product, ProductImage, ProductRequest, UserQuery,
        UserRequest, UserResponse, UserSearchResponse,
    },
};

pub async fn create_product(pool: &PgPool, req: &ProductRequest) -> Result<Product> {
    let product = sqlx::query_as::<_, Product>(
        r#"
        INSERT INTO products (
            id, name, description, price, discount, quantity,
            specifications, brand, warranty, enabled
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING *
        "#,
    )
    .bind(req.id)
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
    .bind(&req.brand)
    .bind(&req.warranty)
    .bind(req.enabled.unwrap_or(true))
    .fetch_one(pool)
    .await?;

    Ok(product)
}

pub async fn update_product(pool: &PgPool, id: i32, req: &ProductRequest) -> Result<Product> {
    let product = sqlx::query_as::<_, Product>(
        r#"
        UPDATE products
        SET
            name = COALESCE($1, name),
            description = COALESCE($2, description),
            price = COALESCE($3, price),
            discount = COALESCE($4, discount),
            quantity = COALESCE($5, quantity),
            specifications = COALESCE($6, specifications),
            brand = COALESCE($7, brand),
            warranty = COALESCE($8, warranty),
            enabled = COALESCE($9, enabled),
            updated_at = NOW()
        WHERE id = $10
        RETURNING *
        "#,
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.price)
    .bind(&req.discount)
    .bind(&req.quantity)
    .bind(&req.specifications)
    .bind(&req.brand)
    .bind(&req.warranty)
    .bind(&req.enabled)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(product)
}

pub async fn delete_product(pool: &PgPool, id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM products WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

pub async fn add_product_image(
    pool: &PgPool,
    product_id: i32,
    image_uuid: uuid::Uuid,
    color: Option<String>,
    is_primary: bool,
    extension: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO product_images(product_id, image_uuid, color, is_primary, extension)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(product_id)
    .bind(image_uuid)
    .bind(color)
    .bind(is_primary)
    .bind(extension)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_product_image(
    pool: &PgPool,
    product_id: i32,
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
    product_id: i32,
    image_uuid: uuid::Uuid,
    color: Option<String>,
    is_primary: Option<bool>,
) -> Result<Option<ProductImage>> {
    let updated_image = sqlx::query_as::<_, ProductImage>(
        r#"
        UPDATE product_images
        SET
            color = COALESCE($3, color),
            is_primary = COALESCE($4, is_primary)
        WHERE product_id = $1 AND image_uuid = $2
        RETURNING *
        "#,
    )
    .bind(product_id)
    .bind(image_uuid)
    .bind(color)
    .bind(is_primary)
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
            crate::models::OrderResponse { order, items }
        })
        .collect();

    Ok(OrderSearchResponse {
        orders,
        total,
        limit,
        offset,
    })
}
