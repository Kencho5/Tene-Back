use crate::models::user::UserRole;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ProductRequest {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<Decimal>,
    pub discount: Option<Decimal>,
    pub quantity: Option<i32>,
    pub specifications: Option<serde_json::Value>,
    pub brand_id: Option<i32>,
    pub warranty: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ImageUploadRequest {
    pub color: Option<String>,
    pub is_primary: bool,
    pub content_type: String,
    pub quantity: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ProductImageUrlRequest {
    pub images: Vec<ImageUploadRequest>,
}

#[derive(Debug, Serialize)]
pub struct ImageUploadUrl {
    pub image_uuid: Uuid,
    pub upload_url: String,
    pub public_url: String,
}

#[derive(Debug, Serialize)]
pub struct ProductImageUrlResponse {
    pub images: Vec<ImageUploadUrl>,
}

#[derive(Debug, Deserialize)]
pub struct ImageMetadataUpdate {
    pub color: Option<String>,
    pub is_primary: Option<bool>,
    pub quantity: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UserRequest {
    pub email: Option<String>,
    pub name: Option<String>,
    pub role: Option<UserRole>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserResponse {
    pub id: i32,
    pub email: String,
    pub name: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserQuery {
    pub id: Option<i32>,
    pub email: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct UserSearchResponse {
    pub users: Vec<UserResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Brand {
    pub id: i32,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct BrandRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderQuery {
    pub id: Option<i32>,
    pub user_id: Option<i32>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct OrderSearchResponse {
    pub orders: Vec<crate::models::OrderResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MostViewedProduct {
    pub product_id: String,
    pub product_name: String,
    pub views: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TrendingProduct {
    pub product_id: String,
    pub product_name: String,
    pub views: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct UniqueViewersProduct {
    pub product_id: String,
    pub product_name: String,
    pub unique_viewers: i64,
    pub total_views: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ViewsByHour {
    pub hour: Decimal,
    pub views: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct HighViewsLowSales {
    pub product_id: String,
    pub product_name: String,
    pub views: i64,
    pub sold: i64,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ConversionRate {
    pub product_id: String,
    pub product_name: String,
    pub viewers: i64,
    pub purchases: i64,
    pub conversion_pct: Decimal,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsResponse {
    pub most_viewed: Vec<MostViewedProduct>,
    pub trending_this_week: Vec<TrendingProduct>,
    pub unique_viewers: Vec<UniqueViewersProduct>,
    pub views_by_hour: Vec<ViewsByHour>,
    pub high_views_low_sales: Vec<HighViewsLowSales>,
    pub conversion_rates: Vec<ConversionRate>,
}
