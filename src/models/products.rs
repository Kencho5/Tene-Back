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
    pub brand: Option<String>,
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
    pub data: Product,
    pub images: Vec<ProductImage>,
}

#[derive(Debug, Serialize)]
pub struct ProductSearchResponse {
    pub products: Vec<ProductResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    PriceAsc,
    PriceDesc,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SaleType {
    Discount,
    Coins,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductQuery {
    pub query: Option<String>,
    pub price_from: Option<i16>,
    pub price_to: Option<i16>,
    pub product_type: Option<String>,
    pub brand: Option<String>,
    #[serde(default)]
    pub color: Vec<String>,
    #[serde(default)]
    pub sale_type: Vec<SaleType>,
    pub sort_by: Option<SortBy>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct FacetValue {
    pub value: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct ProductFacets {
    pub brands: Vec<FacetValue>,
    pub colors: Vec<FacetValue>,
}
