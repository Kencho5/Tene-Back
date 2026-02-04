use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Category {
    pub id: i32,
    pub parent_id: Option<i32>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub display_order: i32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProductCategory {
    pub product_id: i32,
    pub category_id: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CategoryWithChildren {
    #[serde(flatten)]
    pub category: Category,
    pub children: Vec<CategoryWithChildren>,
}

#[derive(Debug, Serialize)]
pub struct CategoryTree {
    pub categories: Vec<CategoryWithChildren>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCategoryRequest {
    pub parent_id: Option<i32>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub display_order: Option<i32>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCategoryRequest {
    pub parent_id: Option<i32>,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub display_order: Option<i32>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CategoryFacetValue {
    pub id: i32,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CategoryImage {
    pub category_id: i32,
    pub image_uuid: Uuid,
    pub extension: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CategoryResponse {
    #[serde(flatten)]
    pub category: Category,
    pub image_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CategoryTreeResponse {
    pub categories: Vec<CategoryResponseWithChildren>,
}

#[derive(Debug, Serialize)]
pub struct CategoryResponseWithChildren {
    #[serde(flatten)]
    pub category: Category,
    pub image_url: Option<String>,
    pub children: Vec<CategoryResponseWithChildren>,
}

#[derive(Debug, Deserialize)]
pub struct CategoryImageUploadRequest {
    pub content_type: String,
}

#[derive(Debug, Serialize)]
pub struct CategoryImageUploadUrl {
    pub image_uuid: Uuid,
    pub upload_url: String,
    pub public_url: String,
}
