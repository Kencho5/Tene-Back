use sqlx::PgPool;

use crate::{
    error::Result,
    models::{Product, ProductImage, ProductRequest},
};

pub async fn create_product(pool: &PgPool, req: &ProductRequest) -> Result<Product> {
    let product = sqlx::query_as::<_, Product>(
        r#"
        INSERT INTO products (
            id, name, description, price, discount, quantity,
            specifications, product_type, brand, warranty
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
    .bind(&req.product_type)
    .bind(&req.brand)
    .bind(&req.warranty)
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
            product_type = COALESCE($7, product_type),
            brand = COALESCE($8, brand),
            warranty = COALESCE($9, warranty),
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
    .bind(&req.product_type)
    .bind(&req.brand)
    .bind(&req.warranty)
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
