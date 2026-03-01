use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// DB models

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Order {
    pub id: i32,
    pub user_id: i32,
    pub order_id: String,
    pub status: String,
    pub payment_id: Option<i32>,
    pub amount: i32,
    pub currency: String,
    pub customer_type: String,
    pub customer_name: Option<String>,
    pub customer_surname: Option<String>,
    pub organization_type: Option<String>,
    pub organization_name: Option<String>,
    pub organization_code: Option<String>,
    pub email: String,
    pub phone_number: i64,
    pub address: String,
    pub delivery_type: String,
    pub delivery_time: String,
    pub checkout_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrderItem {
    pub id: i32,
    pub order_id: i32,
    pub product_id: i32,
    pub color: Option<String>,
    pub quantity: i32,
    pub price_at_purchase: Decimal,
    pub product_name: String,
    pub product_image: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    #[serde(flatten)]
    pub order: Order,
    pub items: Vec<OrderItem>,
}

// Request types

#[derive(Debug, Deserialize)]
pub struct IndividualInfo {
    pub name: String,
    pub surname: String,
}

#[derive(Debug, Deserialize)]
pub struct CompanyInfo {
    pub organization_type: String,
    pub organization_name: String,
    pub organization_code: String,
}

#[derive(Debug, Deserialize)]
pub struct CartItem {
    pub product_id: i32,
    pub color: Option<String>,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutRequest {
    pub customer_type: String,
    pub individual: Option<IndividualInfo>,
    pub company: Option<CompanyInfo>,
    pub email: String,
    pub phone_number: i64,
    pub address: String,
    pub delivery_type: String,
    pub delivery_time: String,
    pub items: Vec<CartItem>,
}

// Response types

#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    pub order_id: String,
    pub checkout_url: String,
}

