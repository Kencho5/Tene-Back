use crate::models::user::UserRole;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ProductRequest {
    pub id: Option<i32>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<Decimal>,
    pub discount: Option<Decimal>,
    pub quantity: Option<i32>,
    pub specifications: Option<serde_json::Value>,
    pub product_type: Option<String>,
    pub brand: Option<String>,
    pub warranty: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImageUploadRequest {
    pub color: Option<String>,
    pub is_primary: bool,
    pub content_type: String,
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
