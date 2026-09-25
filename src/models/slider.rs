use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SliderImageVariant {
    Desktop,
    Mobile,
}

#[derive(Debug, Clone, FromRow)]
pub struct Slider {
    pub id: i32,
    pub title: Option<String>,
    pub link_url: Option<String>,
    pub enabled: bool,
    pub display_order: i32,
    pub desktop_image_uuid: Option<Uuid>,
    pub desktop_image_extension: Option<String>,
    pub mobile_image_uuid: Option<Uuid>,
    pub mobile_image_extension: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Slider {
    pub fn image(&self, variant: SliderImageVariant) -> Option<(Uuid, &str)> {
        match variant {
            SliderImageVariant::Desktop => self
                .desktop_image_uuid
                .zip(self.desktop_image_extension.as_deref()),
            SliderImageVariant::Mobile => self
                .mobile_image_uuid
                .zip(self.mobile_image_extension.as_deref()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SliderResponse {
    pub id: i32,
    pub title: Option<String>,
    pub link_url: Option<String>,
    pub enabled: bool,
    pub display_order: i32,
    pub desktop_image_url: Option<String>,
    pub mobile_image_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SliderRequest {
    pub title: Option<String>,
    pub link_url: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SliderImageUploadRequest {
    pub variant: SliderImageVariant,
    pub content_type: String,
}

#[derive(Debug, Serialize)]
pub struct SliderImageUploadUrl {
    pub image_uuid: Uuid,
    pub upload_url: String,
    pub public_url: String,
}
