use axum::{
    Json,
    extract::{Path, State},
};
use http::StatusCode;
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{
        MoveCategoryRequest, Slider, SliderImageUploadRequest, SliderImageUploadUrl,
        SliderImageVariant, SliderRequest, SliderResponse,
    },
    queries::slider_queries,
    services::image_url_service::{delete_single_object, put_object_url},
};

fn env_prefix(state: &AppState) -> &'static str {
    match state.environment {
        crate::config::Environment::Staging => "sliders-staging",
        crate::config::Environment::Main => "sliders-main",
    }
}

fn image_key(state: &AppState, id: i32, image_uuid: Uuid, extension: &str) -> String {
    format!("{}/{}/{}.{}", env_prefix(state), id, image_uuid, extension)
}

fn to_response(state: &AppState, slider: Slider) -> SliderResponse {
    let image_url = |variant| {
        slider.image(variant).map(|(uuid, ext)| {
            format!(
                "{}/{}",
                state.assets_url,
                image_key(state, slider.id, uuid, ext)
            )
        })
    };

    SliderResponse {
        desktop_image_url: image_url(SliderImageVariant::Desktop),
        mobile_image_url: image_url(SliderImageVariant::Mobile),
        id: slider.id,
        title: slider.title,
        link_url: slider.link_url,
        enabled: slider.enabled,
        display_order: slider.display_order,
        created_at: slider.created_at,
        updated_at: slider.updated_at,
    }
}

fn normalize(mut req: SliderRequest) -> SliderRequest {
    let clean = |value: Option<String>| {
        value
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    req.title = clean(req.title);
    req.link_url = clean(req.link_url);
    req
}

async fn find_slider(state: &AppState, id: i32) -> Result<Slider> {
    slider_queries::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("სლაიდერი id-ით {} ვერ მოიძებნა", id)))
}

async fn delete_image_object(
    state: &AppState,
    id: i32,
    image_uuid: Uuid,
    extension: &str,
) -> Result<()> {
    delete_single_object(
        &state.s3_client,
        &state.s3_bucket,
        &image_key(state, id, image_uuid, extension),
    )
    .await
    .map_err(|e| AppError::InternalError(format!("S3-დან სურათის წაშლა ვერ მოხერხდა: {}", e)))
}

pub async fn get_public_sliders(
    State(state): State<AppState>,
) -> Result<Json<Vec<SliderResponse>>> {
    let sliders = slider_queries::get_public_sliders(&state.db).await?;
    Ok(Json(
        sliders
            .into_iter()
            .map(|s| to_response(&state, s))
            .collect(),
    ))
}

pub async fn get_sliders(State(state): State<AppState>) -> Result<Json<Vec<SliderResponse>>> {
    let sliders = slider_queries::get_sliders(&state.db).await?;
    Ok(Json(
        sliders
            .into_iter()
            .map(|s| to_response(&state, s))
            .collect(),
    ))
}

pub async fn create_slider(
    State(state): State<AppState>,
    Json(payload): Json<SliderRequest>,
) -> Result<Json<SliderResponse>> {
    let slider = slider_queries::create_slider(&state.db, &normalize(payload)).await?;
    Ok(Json(to_response(&state, slider)))
}

pub async fn update_slider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<SliderRequest>,
) -> Result<Json<SliderResponse>> {
    let slider = slider_queries::update_slider(&state.db, id, &normalize(payload))
        .await?
        .ok_or_else(|| AppError::NotFound(format!("სლაიდერი id-ით {} ვერ მოიძებნა", id)))?;
    Ok(Json(to_response(&state, slider)))
}

pub async fn delete_slider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode> {
    let slider = find_slider(&state, id).await?;

    for variant in [SliderImageVariant::Desktop, SliderImageVariant::Mobile] {
        if let Some((uuid, ext)) = slider.image(variant) {
            delete_image_object(&state, id, uuid, ext).await?;
        }
    }

    slider_queries::delete_slider(&state.db, id).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn move_slider(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<MoveCategoryRequest>,
) -> Result<StatusCode> {
    if !slider_queries::move_slider(&state.db, id, payload.direction).await? {
        return Err(AppError::NotFound(format!(
            "სლაიდერი id-ით {} ვერ მოიძებნა",
            id
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn generate_slider_image_url(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<SliderImageUploadRequest>,
) -> Result<Json<SliderImageUploadUrl>> {
    let slider = find_slider(&state, id).await?;

    let extension = match payload.content_type.as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => {
            return Err(AppError::BadRequest(
                "დაშვებულია მხოლოდ JPG, PNG, WEBP ან GIF".to_string(),
            ));
        }
    };

    if let Some((old_uuid, old_ext)) = slider.image(payload.variant) {
        delete_image_object(&state, id, old_uuid, old_ext).await?;
    }

    let image_uuid = Uuid::new_v4();
    let key = image_key(&state, id, image_uuid, extension);

    let upload_url = put_object_url(
        &state.s3_client,
        &state.s3_bucket,
        &key,
        &payload.content_type,
        "public, max-age=31536000, immutable",
        900,
    )
    .await
    .map_err(|e| {
        AppError::InternalError(format!(
            "წინასწარ ხელმოწერილი URL-ის გენერაცია ვერ მოხერხდა: {}",
            e
        ))
    })?;

    slider_queries::set_slider_image(
        &state.db,
        id,
        payload.variant,
        Some((image_uuid, extension)),
    )
    .await?;

    Ok(Json(SliderImageUploadUrl {
        image_uuid,
        upload_url,
        public_url: format!("{}/{}", state.assets_url, key),
    }))
}

pub async fn delete_slider_image(
    State(state): State<AppState>,
    Path((id, variant)): Path<(i32, SliderImageVariant)>,
) -> Result<StatusCode> {
    let slider = find_slider(&state, id).await?;

    let (uuid, ext) = slider
        .image(variant)
        .ok_or_else(|| AppError::NotFound("სლაიდერის სურათი ვერ მოიძებნა".to_string()))?;

    delete_image_object(&state, id, uuid, ext).await?;
    slider_queries::set_slider_image(&state.db, id, variant, None).await?;

    Ok(StatusCode::NO_CONTENT)
}
