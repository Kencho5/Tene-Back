use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BlogStatus {
    Draft,
    Published,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum BlogMediaType {
    Image,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Blog {
    pub id: i32,
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub content: String,
    pub status: BlogStatus,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BlogMedia {
    pub id: i32,
    pub blog_id: i32,
    pub media_uuid: Uuid,
    pub media_type: BlogMediaType,
    pub extension: String,
    pub is_thumbnail: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBlogRequest {
    pub title: String,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub content: String,
    pub status: Option<BlogStatus>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBlogRequest {
    pub title: Option<String>,
    pub slug: Option<String>,
    pub excerpt: Option<String>,
    pub content: Option<String>,
    pub status: Option<BlogStatus>,
}

#[derive(Debug, Deserialize)]
pub struct BlogQuery {
    pub status: Option<BlogStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PublicBlogQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct BlogWithMedia {
    #[serde(flatten)]
    pub blog: Blog,
    pub thumbnail: Option<BlogMediaResponse>,
    pub media: Vec<BlogMediaResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlogMediaResponse {
    pub media_uuid: Uuid,
    pub media_type: BlogMediaType,
    pub is_thumbnail: bool,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct BlogSearchResponse {
    pub blogs: Vec<BlogWithMedia>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct BlogMediaUploadItem {
    pub media_type: BlogMediaType,
    pub content_type: String,
    #[serde(default)]
    pub is_thumbnail: bool,
}

#[derive(Debug, Deserialize)]
pub struct BlogMediaUploadRequest {
    pub items: Vec<BlogMediaUploadItem>,
}

#[derive(Debug, Serialize)]
pub struct BlogMediaUploadUrl {
    pub media_uuid: Uuid,
    pub media_type: BlogMediaType,
    pub is_thumbnail: bool,
    pub upload_url: String,
    pub public_url: String,
}

#[derive(Debug, Deserialize)]
pub struct BlogMediaThumbnailRequest {
    pub is_thumbnail: bool,
}

#[derive(Debug, Serialize)]
pub struct BlogMediaUploadResponse {
    pub media: Vec<BlogMediaUploadUrl>,
}
