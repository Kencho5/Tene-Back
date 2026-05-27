use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{
    error::Result,
    models::{CheckoutRequest, CustomerInfo, Order, OrderItem, OrderItemData},
};

pub struct OrderContact<'a> {
    pub customer: &'a CustomerInfo,
    pub email: &'a str,
    pub phone_number: &'a str,
    pub address: &'a str,
    pub city: Option<&'a str>,
    pub details: Option<&'a str>,
    pub delivery_type: &'a str,
    pub delivery_time: &'a str,
    pub comment: Option<&'a str>,
}

pub async fn create_order_with_items(
    pool: &PgPool,
    user_id: Option<i32>,
    order_id: &str,
    amount: i32,
    req: &CheckoutRequest,
    items: &[OrderItemData],
) -> Result<Order> {
    let contact = OrderContact {
        customer: &req.customer,
        email: &req.email,
        phone_number: &req.phone_number,
        address: &req.address,
        city: req.city.as_deref(),
        details: req.details.as_deref(),
        delivery_type: &req.delivery_type,
        delivery_time: &req.delivery_time,
        comment: req.comment.as_deref(),
    };
    create_order_with_items_raw(pool, user_id, order_id, amount, &contact, items).await
}

pub async fn create_order_with_items_raw(
    pool: &PgPool,
    user_id: Option<i32>,
    order_id: &str,
    amount: i32,
    contact: &OrderContact<'_>,
    items: &[OrderItemData],
) -> Result<Order> {
    let mut tx = pool.begin().await?;

    let (customer_type, customer_name, customer_surname, org_type, org_name, org_code) =
        match contact.customer {
            CustomerInfo::Individual { name, surname } => (
                "individual",
                Some(name.as_str()),
                Some(surname.as_str()),
                None,
                None,
                None,
            ),
            CustomerInfo::Company {
                organization_type,
                organization_name,
                organization_code,
            } => (
                "company",
                None,
                None,
                Some(organization_type.as_str()),
                Some(organization_name.as_str()),
                Some(organization_code.as_str()),
            ),
        };

    let order = sqlx::query_as::<_, Order>(
        "INSERT INTO orders (user_id, order_id, amount, customer_type, customer_name, customer_surname,
         organization_type, organization_name, organization_code, email, phone_number, address,
         city, details, delivery_type, delivery_time, comment)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
         RETURNING *",
    )
    .bind(user_id)
    .bind(order_id)
    .bind(amount)
    .bind(customer_type)
    .bind(customer_name)
    .bind(customer_surname)
    .bind(org_type)
    .bind(org_name)
    .bind(org_code)
    .bind(contact.email)
    .bind(contact.phone_number)
    .bind(contact.address)
    .bind(contact.city)
    .bind(contact.details)
    .bind(contact.delivery_type)
    .bind(contact.delivery_time)
    .bind(contact.comment)
    .fetch_one(&mut *tx)
    .await?;

    let product_ids: Vec<&str> = items.iter().map(|i| i.product_id.as_str()).collect();
    let colors: Vec<Option<&str>> = items.iter().map(|i| i.color.as_deref()).collect();
    let quantities: Vec<i32> = items.iter().map(|i| i.quantity).collect();
    let prices: Vec<Decimal> = items.iter().map(|i| i.price).collect();
    let product_names: Vec<&str> = items.iter().map(|i| i.product_name.as_str()).collect();
    let product_images: Vec<serde_json::Value> = items.iter().map(|i| i.image.clone()).collect();
    let cable_configs: Vec<Option<serde_json::Value>> =
        items.iter().map(|i| i.cable_config.clone()).collect();

    sqlx::query(
        "INSERT INTO order_items (order_id, product_id, color, quantity, price_at_purchase, product_name, product_image, cable_config)
         SELECT $1, unnest($2::text[]), unnest($3::varchar[]), unnest($4::int[]), unnest($5::decimal[]), unnest($6::varchar[]), unnest($7::jsonb[]), unnest($8::jsonb[])",
    )
    .bind(order.id)
    .bind(&product_ids)
    .bind(&colors)
    .bind(&quantities)
    .bind(&prices)
    .bind(&product_names)
    .bind(&product_images)
    .bind(&cable_configs)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(order)
}

pub async fn update_order_status_and_deduct_stock(
    pool: &PgPool,
    order_id: &str,
    status: &str,
    payment_id: Option<i32>,
) -> Result<Option<(Order, bool)>> {
    let mut tx = pool.begin().await?;

    let order = sqlx::query_as::<_, Order>(
        "UPDATE orders SET status = $1, payment_id = $2, updated_at = NOW()
         WHERE order_id = $3 AND status <> 'approved' RETURNING *",
    )
    .bind(status)
    .bind(payment_id)
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?;

    let order = match order {
        Some(o) => o,
        None => {
            tx.commit().await?;
            return Ok(None);
        }
    };

    let mut stock_ok = true;

    if status == "approved" {
        let items = sqlx::query_as::<_, OrderItem>(
            "SELECT * FROM order_items WHERE order_id = $1",
        )
        .bind(order.id)
        .fetch_all(&mut *tx)
        .await?;

        for item in &items {
            let Some(product_id) = &item.product_id else {
                continue;
            };
            let result = match &item.color {
                Some(color) => {
                    sqlx::query(
                        "UPDATE product_images
                         SET quantity = quantity - $1
                         WHERE product_id = $2 AND color = $3 AND quantity >= $1",
                    )
                    .bind(item.quantity)
                    .bind(product_id)
                    .bind(color)
                    .execute(&mut *tx)
                    .await?
                }
                None => {
                    sqlx::query(
                        "UPDATE product_images
                         SET quantity = quantity - $1
                         WHERE product_id = $2 AND is_primary = true AND quantity >= $1",
                    )
                    .bind(item.quantity)
                    .bind(product_id)
                    .execute(&mut *tx)
                    .await?
                }
            };

            if result.rows_affected() == 0 {
                stock_ok = false;
                break;
            }
        }

        if !stock_ok {
            tx.rollback().await?;
            return Ok(Some((order, false)));
        }
    }

    tx.commit().await?;
    Ok(Some((order, stock_ok)))
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

pub async fn get_user_order(pool: &PgPool, user_id: i32, order_id: &str) -> Result<Option<Order>> {
    let order = sqlx::query_as::<_, Order>(
        "SELECT * FROM orders WHERE user_id = $1 AND order_id = $2 AND status != 'pending'",
    )
    .bind(user_id)
    .bind(order_id)
    .fetch_optional(pool)
    .await?;

    Ok(order)
}

pub async fn get_order_by_order_id(pool: &PgPool, order_id: &str) -> Result<Option<Order>> {
    let order = sqlx::query_as::<_, Order>(
        "SELECT * FROM orders WHERE order_id = $1 AND status != 'pending'",
    )
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


