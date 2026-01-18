use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Product {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
    pub price: Decimal,
    pub discount: Decimal,
    pub quantity: i32,
    pub specifications: serde_json::Value,
    pub product_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProductImage {
    pub product_id: i32,
    pub image_uuid: Uuid,
    pub color: Option<String>,
    pub is_primary: bool,
}

#[derive(Debug, Serialize)]
pub struct ProductResponse {
    pub product: Product,
    pub images: Vec<ProductImage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductQuery {
    pub query: Option<String>,
    pub price_from: Option<i16>,
    pub price_to: Option<i16>,
    pub sale_type: Option<String>,
    pub brand: Option<String>,
}
