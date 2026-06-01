use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct CheckoutAnalyticsEvent {
    pub session_id: Uuid,
    #[serde(rename = "type")]
    pub event_type: String,
    pub step: Option<String>,
    pub step_index: Option<i32>,
    pub field: Option<String>,
    pub value: Option<String>,
    pub order_id: Option<String>,
    pub is_guest: Option<bool>,
    pub timestamp: Option<i64>,
    pub cart: Option<Vec<CartSnapshotItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartSnapshotItem {
    pub product_id: String,
    pub quantity: i32,
    pub color: Option<String>,
    pub cable_config: Option<CableConfig>,
    pub name: Option<String>,
    pub image_uuid: Option<String>,
    pub image_extension: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutSessionQuery {
    pub session_id: Option<Uuid>,
    pub user_id: Option<i32>,
    pub step: Option<String>,
    pub outcome: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CheckoutEventRow {
    pub id: i64,
    pub session_id: Uuid,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub event_type: String,
    pub step: Option<String>,
    pub step_index: Option<i32>,
    pub field: Option<String>,
    pub value: Option<String>,
    pub order_id: Option<String>,
    pub is_guest: Option<bool>,
    pub user_id: Option<i32>,
    pub client_timestamp: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CheckoutSessionSummary {
    pub session_id: Uuid,
    pub user_id: Option<i32>,
    pub is_guest: Option<bool>,
    pub last_step: Option<String>,
    pub last_step_index: Option<i32>,
    pub purchased: bool,
    pub order_id: Option<String>,
    pub order_status: Option<String>,
    pub event_count: i64,
    pub started_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub fields: std::collections::HashMap<String, String>,
    pub cart: Option<Vec<CartSnapshotItem>>,
    pub events: Vec<CheckoutEventRow>,
}

#[derive(Debug, Serialize)]
pub struct CheckoutSessionsResponse {
    pub sessions: Vec<CheckoutSessionSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Order {
    pub id: i32,
    pub user_id: Option<i32>,
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
    pub phone_number: String,
    pub address: String,
    pub city: Option<String>,
    pub details: Option<String>,
    pub delivery_type: String,
    pub delivery_time: String,
    pub comment: Option<String>,
    pub checkout_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrderItem {
    pub id: i32,
    pub order_id: i32,
    pub product_id: Option<String>,
    pub color: Option<String>,
    pub quantity: i32,
    pub price_at_purchase: Decimal,
    pub product_name: String,
    pub product_image: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cable_config: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CableConfig {
    pub watts: i32,
    pub length_cm: i32,
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    #[serde(flatten)]
    pub order: Order,
    pub items: Vec<OrderItem>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "customer_type", rename_all = "snake_case")]
pub enum CustomerInfo {
    Individual {
        name: String,
        surname: String,
    },
    Company {
        organization_type: String,
        organization_name: String,
        organization_code: String,
    },
}

#[derive(Debug, Deserialize)]
pub struct CartItem {
    pub product_id: String,
    pub color: Option<String>,
    pub quantity: i32,
    pub cable_config: Option<CableConfig>,
}

#[derive(Debug, Deserialize)]
pub struct CheckoutRequest {
    #[serde(flatten)]
    pub customer: CustomerInfo,
    pub email: String,
    pub phone_number: String,
    pub address: String,
    pub city: Option<String>,
    pub details: Option<String>,
    pub delivery_type: String,
    pub delivery_time: String,
    pub comment: Option<String>,
    pub items: Vec<CartItem>,
}

pub struct OrderItemData {
    pub product_id: String,
    pub color: Option<String>,
    pub quantity: i32,
    pub price: Decimal,
    pub product_name: String,
    pub image: serde_json::Value,
    pub cable_config: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    pub order_id: String,
    pub checkout_url: String,
}

#[derive(Debug, Deserialize)]
pub struct PaymentLinkItem {
    pub product_id: String,
    pub product_name: String,
    pub color: Option<String>,
    pub quantity: i32,
    pub price: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct PaymentLinkRequest {
    #[serde(flatten)]
    pub customer: CustomerInfo,
    pub email: String,
    pub phone_number: String,
    pub address: String,
    pub city: Option<String>,
    pub details: Option<String>,
    pub delivery_type: String,
    pub delivery_time: String,
    pub comment: Option<String>,
    pub items: Vec<PaymentLinkItem>,
}

