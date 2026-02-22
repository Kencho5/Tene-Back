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
    pub quantity: i32,
    pub price_at_purchase: Decimal,
    pub created_at: DateTime<Utc>,
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

// Flitt callback payload

#[derive(Debug, Deserialize)]
pub struct FlittCallbackPayload {
    pub order_id: Option<String>,
    pub order_status: Option<String>,
    pub payment_id: Option<i32>,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub signature: Option<String>,
    pub response_status: Option<String>,
    pub merchant_id: Option<i32>,
    pub masked_card: Option<String>,
    pub card_bin: Option<serde_json::Value>,
    pub card_type: Option<String>,
    pub rrn: Option<String>,
    pub approval_code: Option<String>,
    pub sender_email: Option<String>,
    pub sender_cell_phone: Option<String>,
    pub sender_account: Option<String>,
    pub fee: Option<String>,
    pub payment_system: Option<String>,
    pub eci: Option<String>,
    pub actual_amount: Option<String>,
    pub actual_currency: Option<String>,
    pub product_id: Option<String>,
    pub merchant_data: Option<String>,
    pub verification_status: Option<String>,
    pub rectoken: Option<String>,
    pub rectoken_lifetime: Option<String>,
    pub reversal_amount: Option<String>,
    pub settlement_amount: Option<String>,
    pub settlement_currency: Option<String>,
    pub settlement_date: Option<String>,
    pub response_description: Option<String>,
    pub response_code: Option<String>,
    pub order_time: Option<String>,
    pub tran_type: Option<String>,
    pub parent_order_id: Option<String>,
    pub fee_oplata: Option<String>,
    pub additional_info: Option<String>,
    pub response_signature_string: Option<String>,
}
