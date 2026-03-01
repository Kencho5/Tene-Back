use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{
    error::Result,
    models::{CheckoutRequest, Order, OrderItem},
};

pub async fn create_order_with_items(
    pool: &PgPool,
    user_id: i32,
    order_id: &str,
    amount: i32,
    req: &CheckoutRequest,
    product_ids: &[i32],
    colors: &[Option<String>],
    quantities: &[i32],
    prices: &[Decimal],
    product_names: &[String],
    product_images: &[serde_json::Value],
) -> Result<Order> {
    let mut tx = pool.begin().await?;

    let (customer_name, customer_surname) = match &req.individual {
        Some(info) => (Some(info.name.as_str()), Some(info.surname.as_str())),
        None => (None, None),
    };

    let (org_type, org_name, org_code) = match &req.company {
        Some(info) => (
            Some(info.organization_type.as_str()),
            Some(info.organization_name.as_str()),
            Some(info.organization_code.as_str()),
        ),
        None => (None, None, None),
    };

    let order = sqlx::query_as::<_, Order>(
        "INSERT INTO orders (user_id, order_id, amount, customer_type, customer_name, customer_surname,
         organization_type, organization_name, organization_code, email, phone_number, address,
         delivery_type, delivery_time)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
         RETURNING *",
    )
    .bind(user_id)
    .bind(order_id)
    .bind(amount)
    .bind(&req.customer_type)
    .bind(customer_name)
    .bind(customer_surname)
    .bind(org_type)
    .bind(org_name)
    .bind(org_code)
    .bind(&req.email)
    .bind(req.phone_number)
    .bind(&req.address)
    .bind(&req.delivery_type)
    .bind(&req.delivery_time)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO order_items (order_id, product_id, color, quantity, price_at_purchase, product_name, product_image)
         SELECT $1, unnest($2::int[]), unnest($3::varchar[]), unnest($4::int[]), unnest($5::decimal[]), unnest($6::varchar[]), unnest($7::jsonb[])",
    )
    .bind(order.id)
    .bind(product_ids)
    .bind(colors)
    .bind(quantities)
    .bind(prices)
    .bind(product_names)
    .bind(product_images)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(order)
}

pub async fn update_order_status(
    pool: &PgPool,
    order_id: &str,
    status: &str,
    payment_id: Option<i32>,
) -> Result<Option<Order>> {
    // Only update if still pending - prevents double-processing repeated callbacks
    let order = sqlx::query_as::<_, Order>(
        "UPDATE orders SET status = $1, payment_id = $2, updated_at = NOW()
         WHERE order_id = $3 AND status = 'pending' RETURNING *",
    )
    .bind(status)
    .bind(payment_id)
    .bind(order_id)
    .fetch_optional(pool)
    .await?;

    Ok(order)
}

pub async fn update_order_checkout_url(
    pool: &PgPool,
    order_id: &str,
    checkout_url: &str,
) -> Result<Option<Order>> {
    let order = sqlx::query_as::<_, Order>(
        "UPDATE orders SET checkout_url = $1, updated_at = NOW()
         WHERE order_id = $2 RETURNING *",
    )
    .bind(checkout_url)
    .bind(order_id)
    .fetch_optional(pool)
    .await?;

    Ok(order)
}

pub async fn get_user_orders(pool: &PgPool, user_id: i32) -> Result<Vec<Order>> {
    let orders = sqlx::query_as::<_, Order>(
        "SELECT * FROM orders WHERE user_id = $1 AND status != 'pending' ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(orders)
}

pub async fn get_items_for_orders(pool: &PgPool, order_db_ids: &[i32]) -> Result<Vec<OrderItem>> {
    let items =
        sqlx::query_as::<_, OrderItem>("SELECT * FROM order_items WHERE order_id = ANY($1)")
            .bind(order_db_ids)
            .fetch_all(pool)
            .await?;

    Ok(items)
}

pub async fn deduct_stock_for_order(pool: &PgPool, order_db_id: i32) -> Result<bool> {
    let mut tx = pool.begin().await?;

    // Get the order items we need to deduct
    let items = sqlx::query_as::<_, OrderItem>(
        "SELECT * FROM order_items WHERE order_id = $1",
    )
    .bind(order_db_id)
    .fetch_all(&mut *tx)
    .await?;

    if items.is_empty() {
        tx.commit().await?;
        return Ok(true);
    }

    // Deduct each item individually so we can verify each one succeeded
    for item in &items {
        let result = match &item.color {
            Some(color) => {
                // Deduct from the specific color variant
                sqlx::query(
                    "UPDATE product_images
                     SET quantity = quantity - $1
                     WHERE product_id = $2 AND color = $3 AND quantity >= $1",
                )
                .bind(item.quantity)
                .bind(item.product_id)
                .bind(color)
                .execute(&mut *tx)
                .await?
            }
            None => {
                // No color specified — deduct from the primary image
                sqlx::query(
                    "UPDATE product_images
                     SET quantity = quantity - $1
                     WHERE product_id = $2 AND is_primary = true AND quantity >= $1",
                )
                .bind(item.quantity)
                .bind(item.product_id)
                .execute(&mut *tx)
                .await?
            }
        };

        if result.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(false);
        }
    }

    tx.commit().await?;
    Ok(true)
}

