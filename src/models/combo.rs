use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::models::{Product, ProductImage};

pub const COMBO_ADDON_PRICE: &str = "addon_price";
pub const COMBO_BUNDLE_PRICE: &str = "bundle_price";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComboPricingType {
    AddonPrice,
    BundlePrice,
}

impl ComboPricingType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComboPricingType::AddonPrice => COMBO_ADDON_PRICE,
            ComboPricingType::BundlePrice => COMBO_BUNDLE_PRICE,
        }
    }
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ComboOffer {
    pub id: i32,
    pub main_product_id: String,
    pub title: String,
    pub description: Option<String>,
    pub pricing_type: String,
    pub bundle_price: Option<Decimal>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ComboOfferItem {
    pub combo_offer_id: i32,
    pub product_id: String,
    pub combo_price: Option<Decimal>,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComboProduct {
    pub data: Product,
    pub images: Vec<ProductImage>,
}

#[derive(Debug, Serialize)]
pub struct ComboOfferItemResponse {
    pub product_id: String,
    pub combo_price: Option<Decimal>,
    pub position: i32,
    pub product: Option<ComboProduct>,
}

#[derive(Debug, Serialize)]
pub struct ComboOfferResponse {
    #[serde(flatten)]
    pub offer: ComboOffer,
    pub main_product: Option<ComboProduct>,
    pub items: Vec<ComboOfferItemResponse>,
}

#[derive(Debug, Deserialize)]
pub struct ComboOfferItemRequest {
    pub product_id: String,
    #[serde(default)]
    pub combo_price: Option<Decimal>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct ComboOfferRequest {
    pub main_product_id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub pricing_type: ComboPricingType,
    #[serde(default)]
    pub bundle_price: Option<Decimal>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub items: Vec<ComboOfferItemRequest>,
}

#[derive(Debug, Deserialize)]
pub struct ComboOfferEnabledRequest {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct ComboOfferQuery {
    pub main_product_id: Option<String>,
}
