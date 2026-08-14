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
    pub region: Option<String>,
    pub details: Option<String>,
    pub delivery_type: String,
    pub delivery_time: String,
    pub comment: Option<String>,
    pub checkout_url: Option<String>,
    pub source: String,
    pub created_by_user_id: Option<i32>,
    pub payment_method: Option<String>,
    pub fulfillment_method: Option<String>,
    pub personal_number: Option<String>,
    pub source_comment: Option<String>,
    pub is_installment_sale: bool,
    pub is_product_exchange: bool,
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

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OrderCommentImage {
    pub id: i32,
    pub order_id: Option<i32>,
    pub image_uuid: Uuid,
    pub extension: String,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct OrderResponse {
    #[serde(flatten)]
    pub order: Order,
    pub items: Vec<OrderItem>,
    pub comment_images: Vec<CommentImage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<OrderCreator>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OrderCreator {
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct CommentImage {
    pub image_uuid: Uuid,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct CommentImageUploadRequest {
    pub content_type: String,
}

#[derive(Debug, Deserialize)]
pub struct CommentImageUrlRequest {
    pub images: Vec<CommentImageUploadRequest>,
}

#[derive(Debug, Serialize)]
pub struct CommentImageUploadUrl {
    pub image_uuid: Uuid,
    pub upload_url: String,
    pub public_url: String,
}

#[derive(Debug, Serialize)]
pub struct CommentImageUrlResponse {
    pub images: Vec<CommentImageUploadUrl>,
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
    #[serde(default)]
    pub region: Option<String>,
    pub details: Option<String>,
    pub delivery_type: String,
    pub delivery_time: String,
    pub comment: Option<String>,
    pub items: Vec<CartItem>,
    #[serde(default)]
    pub comment_image_uuids: Vec<Uuid>,
    pub payment_method: CheckoutPaymentMethod,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckoutPaymentMethod {
    Card,
    CashOnDelivery,
}

impl CheckoutPaymentMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckoutPaymentMethod::Card => "card",
            CheckoutPaymentMethod::CashOnDelivery => "cash_on_delivery",
        }
    }
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
    pub checkout_url: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PaymentMethod {
    Pos,
    PosBog,
    PosTbc,
    PosLiberty,
    Cash,
    Transfer,
    TransferBog,
    TransferTbc,
    TransferExtra,
    Card,
    CashOnDelivery,
}

impl PaymentMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentMethod::Pos => "pos",
            PaymentMethod::PosBog => "pos_bog",
            PaymentMethod::PosTbc => "pos_tbc",
            PaymentMethod::PosLiberty => "pos_liberty",
            PaymentMethod::Cash => "cash",
            PaymentMethod::Transfer => "transfer",
            PaymentMethod::TransferBog => "transfer_bog",
            PaymentMethod::TransferTbc => "transfer_tbc",
            PaymentMethod::TransferExtra => "transfer_extra",
            PaymentMethod::Card => "card",
            PaymentMethod::CashOnDelivery => "cash_on_delivery",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FulfillmentMethod {
    StorePickup,
    Courier,
}

impl FulfillmentMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            FulfillmentMethod::StorePickup => "store_pickup",
            FulfillmentMethod::Courier => "courier",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrderSource {
    Web,
    Admin,
}

impl OrderSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderSource::Web => "web",
            OrderSource::Admin => "admin",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AdminOrderItemRequest {
    #[serde(default)]
    pub product_id: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub quantity: Option<i32>,
    #[serde(default)]
    pub price: Option<Decimal>,
    #[serde(default)]
    pub product_name: Option<String>,
    #[serde(default)]
    pub cable_config: Option<CableConfig>,
}

#[derive(Debug, Deserialize)]
pub struct AdminOrderRequest {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub amount: Option<Decimal>,
    #[serde(default)]
    pub customer_type: Option<String>,
    #[serde(default)]
    pub customer_name: Option<String>,
    #[serde(default)]
    pub customer_surname: Option<String>,
    #[serde(default)]
    pub organization_type: Option<String>,
    #[serde(default)]
    pub organization_name: Option<String>,
    #[serde(default)]
    pub organization_code: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone_number: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub delivery_type: Option<String>,
    #[serde(default)]
    pub delivery_time: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub user_id: Option<i32>,
    #[serde(default)]
    pub payment_method: Option<PaymentMethod>,
    #[serde(default)]
    pub fulfillment_method: Option<FulfillmentMethod>,
    #[serde(default)]
    pub personal_number: Option<String>,
    #[serde(default)]
    pub source_comment: Option<String>,
    #[serde(default)]
    pub is_installment_sale: bool,
    #[serde(default)]
    pub is_product_exchange: bool,
    #[serde(default)]
    pub items: Vec<AdminOrderItemRequest>,
    #[serde(default)]
    pub comment_image_uuids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PaymentLinkRequest {
    #[serde(default)]
    pub price: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone_number: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
}

