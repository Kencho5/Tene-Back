use axum::{
    Json,
    extract::{Path, Query, State},
};
use http::StatusCode;
use std::collections::HashMap;
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{
        Blog, BlogMediaResponse, BlogMediaType, BlogMediaUploadRequest, BlogMediaUploadResponse,
        BlogMediaThumbnailRequest, BlogMediaUploadUrl, BlogQuery, BlogSearchResponse,
        BlogWithMedia, CreateBlogRequest, PublicBlogQuery, UpdateBlogRequest,
    },
    queries::blog_queries,
    services::image_url_service::{delete_objects_by_prefix, delete_single_object, put_object_url},
};

fn env_prefix(state: &AppState) -> &'static str {
    match state.environment {
        crate::config::Environment::Staging => "blogs-staging",
        crate::config::Environment::Main => "blogs-main",
    }
}

fn ext_for(media_type: &BlogMediaType, content_type: &str) -> &'static str {
    match media_type {
        BlogMediaType::Image => match content_type {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            "image/gif" => "gif",
            _ => "jpg",
        },
        BlogMediaType::Video => match content_type {
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            "video/quicktime" => "mov",
            _ => "mp4",
        },
    }
}

fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut prev_dash = false;
    for ch in input.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                slug.push(lower);
            }
            prev_dash = false;
        } else if !prev_dash && !slug.is_empty() {
            slug.push('-');
            prev_dash = true;
        }
    }
    slug.trim_matches('-').chars().take(280).collect()
}

async fn unique_slug(
    state: &AppState,
    desired: &str,
    exclude_id: Option<i32>,
) -> Result<String> {
    let base = slugify(desired);
    let base = if base.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        base
    };

    let mut candidate = base.clone();
    let mut suffix = 2;
    while blog_queries::slug_exists(&state.db, &candidate, exclude_id).await? {
        candidate = format!("{}-{}", base, suffix);
        suffix += 1;
    }
    Ok(candidate)
}

fn media_url(state: &AppState, blog_id: i32, m: &crate::models::BlogMedia) -> String {
    format!(
        "{}/{}/{}/{}.{}",
        state.assets_url,
        env_prefix(state),
        blog_id,
        m.media_uuid,
        m.extension
    )
}

fn to_response(state: &AppState, blog_id: i32, m: &crate::models::BlogMedia) -> BlogMediaResponse {
    BlogMediaResponse {
        url: media_url(state, blog_id, m),
        media_uuid: m.media_uuid,
        media_type: m.media_type.clone(),
        is_thumbnail: m.is_thumbnail,
    }
}

async fn load_blog_with_media(state: &AppState, blog: Blog) -> Result<BlogWithMedia> {
    let media = blog_queries::get_blog_media(&state.db, blog.id).await?;
    let responses: Vec<BlogMediaResponse> =
        media.iter().map(|m| to_response(state, blog.id, m)).collect();
    let thumbnail = responses.iter().find(|m| m.is_thumbnail).cloned();
    Ok(BlogWithMedia {
        blog,
        thumbnail,
        media: responses,
    })
}

pub async fn create_blog(
    State(state): State<AppState>,
    Json(payload): Json<CreateBlogRequest>,
) -> Result<Json<BlogWithMedia>> {
    if payload.title.trim().is_empty() {
        return Err(AppError::BadRequest("title აუცილებელია".to_string()));
    }
    if payload.content.trim().is_empty() {
        return Err(AppError::BadRequest("content აუცილებელია".to_string()));
    }

    let desired = payload.slug.as_deref().unwrap_or(&payload.title);
    let slug = unique_slug(&state, desired, None).await?;

    let blog = blog_queries::create_blog(&state.db, &payload, &slug).await?;
    let resp = load_blog_with_media(&state, blog).await?;
    Ok(Json(resp))
}

pub async fn get_blog(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<BlogWithMedia>> {
    let blog = blog_queries::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("blog id-ით {} ვერ მოიძებნა", id)))?;
    let resp = load_blog_with_media(&state, blog).await?;
    Ok(Json(resp))
}

pub async fn search_blogs(
    State(state): State<AppState>,
    Query(params): Query<BlogQuery>,
) -> Result<Json<BlogSearchResponse>> {
    let (blogs, total, limit, offset) = blog_queries::search_blogs(&state.db, params).await?;

    let ids: Vec<i32> = blogs.iter().map(|b| b.id).collect();
    let media_rows = blog_queries::get_media_for_blogs(&state.db, &ids).await?;
    let blogs = assemble_blogs_with_media(&state, blogs, media_rows);

    Ok(Json(BlogSearchResponse {
        blogs,
        total,
        limit,
        offset,
    }))
}

fn assemble_blogs_with_media(
    state: &AppState,
    blogs: Vec<Blog>,
    media_rows: Vec<crate::models::BlogMedia>,
) -> Vec<BlogWithMedia> {
    let mut map: HashMap<i32, Vec<BlogMediaResponse>> = HashMap::new();
    for m in &media_rows {
        map.entry(m.blog_id)
            .or_default()
            .push(to_response(state, m.blog_id, m));
    }

    blogs
        .into_iter()
        .map(|b| {
            let media = map.remove(&b.id).unwrap_or_default();
            let thumbnail = media.iter().find(|m| m.is_thumbnail).cloned();
            BlogWithMedia {
                blog: b,
                thumbnail,
                media,
            }
        })
        .collect()
}

pub async fn list_public_blogs(
    State(state): State<AppState>,
    Query(params): Query<PublicBlogQuery>,
) -> Result<Json<BlogSearchResponse>> {
    let (blogs, total, limit, offset) = blog_queries::list_published(
        &state.db,
        params.limit.unwrap_or(20),
        params.offset.unwrap_or(0),
    )
    .await?;

    let ids: Vec<i32> = blogs.iter().map(|b| b.id).collect();
    let media_rows = blog_queries::get_media_for_blogs(&state.db, &ids).await?;
    let blogs = assemble_blogs_with_media(&state, blogs, media_rows);

    Ok(Json(BlogSearchResponse {
        blogs,
        total,
        limit,
        offset,
    }))
}

pub async fn get_public_blog(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<BlogWithMedia>> {
    let blog = blog_queries::find_published_by_slug(&state.db, &slug)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("blog '{}' ვერ მოიძებნა", slug)))?;
    let resp = load_blog_with_media(&state, blog).await?;
    Ok(Json(resp))
}

pub async fn update_blog(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateBlogRequest>,
) -> Result<Json<BlogWithMedia>> {
    if blog_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!("blog id-ით {} ვერ მოიძებნა", id)));
    }

    let slug = match payload.slug.as_deref() {
        Some(s) => Some(unique_slug(&state, s, Some(id)).await?),
        None => None,
    };

    let blog = blog_queries::update_blog(&state.db, id, &payload, slug.as_deref()).await?;
    let resp = load_blog_with_media(&state, blog).await?;
    Ok(Json(resp))
}

pub async fn delete_blog(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode> {
    if blog_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!("blog id-ით {} ვერ მოიძებნა", id)));
    }

    let prefix = format!("{}/{}/", env_prefix(&state), id);
    delete_objects_by_prefix(&state.s3_client, &state.s3_bucket, &prefix)
        .await
        .map_err(|e| {
            AppError::InternalError(format!("S3-დან მედიის წაშლა ვერ მოხერხდა: {}", e))
        })?;

    blog_queries::delete_blog(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn generate_blog_media_urls(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<BlogMediaUploadRequest>,
) -> Result<Json<BlogMediaUploadResponse>> {
    if blog_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!("blog id-ით {} ვერ მოიძებნა", id)));
    }

    let mut out = Vec::with_capacity(payload.items.len());

    for item in payload.items {
        if item.is_thumbnail && !matches!(item.media_type, BlogMediaType::Image) {
            return Err(AppError::BadRequest(
                "thumbnail მხოლოდ სურათი შეიძლება იყოს".to_string(),
            ));
        }

        let media_uuid = Uuid::new_v4();
        let extension = ext_for(&item.media_type, &item.content_type);
        let key = format!("{}/{}/{}.{}", env_prefix(&state), id, media_uuid, extension);

        let upload_url = put_object_url(
            &state.s3_client,
            &state.s3_bucket,
            &key,
            &item.content_type,
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

        let public_url = format!("{}/{}", state.assets_url, key);

        blog_queries::add_blog_media(
            &state.db,
            id,
            media_uuid,
            &item.media_type,
            extension,
            item.is_thumbnail,
        )
        .await?;

        out.push(BlogMediaUploadUrl {
            media_uuid,
            media_type: item.media_type,
            is_thumbnail: item.is_thumbnail,
            upload_url,
            public_url,
        });
    }

    Ok(Json(BlogMediaUploadResponse { media: out }))
}

pub async fn set_blog_media_thumbnail(
    State(state): State<AppState>,
    Path((blog_id, media_uuid)): Path<(i32, Uuid)>,
    Json(payload): Json<BlogMediaThumbnailRequest>,
) -> Result<Json<BlogWithMedia>> {
    let existing = blog_queries::get_blog_media(&state.db, blog_id)
        .await?
        .into_iter()
        .find(|m| m.media_uuid == media_uuid)
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "მედია {} ვერ მოიძებნა blog {}-ისთვის",
                media_uuid, blog_id
            ))
        })?;

    if payload.is_thumbnail && !matches!(existing.media_type, BlogMediaType::Image) {
        return Err(AppError::BadRequest(
            "thumbnail მხოლოდ სურათი შეიძლება იყოს".to_string(),
        ));
    }

    blog_queries::set_blog_media_thumbnail(&state.db, blog_id, media_uuid, payload.is_thumbnail)
        .await?;

    let blog = blog_queries::find_by_id(&state.db, blog_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("blog id-ით {} ვერ მოიძებნა", blog_id)))?;
    let resp = load_blog_with_media(&state, blog).await?;
    Ok(Json(resp))
}

pub async fn delete_blog_media(
    State(state): State<AppState>,
    Path((blog_id, media_uuid)): Path<(i32, Uuid)>,
) -> Result<StatusCode> {
    let deleted = blog_queries::delete_blog_media(&state.db, blog_id, media_uuid)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "მედია {} ვერ მოიძებნა blog {}-ისთვის",
                media_uuid, blog_id
            ))
        })?;

    let key = format!(
        "{}/{}/{}.{}",
        env_prefix(&state),
        blog_id,
        deleted.media_uuid,
        deleted.extension
    );

    delete_single_object(&state.s3_client, &state.s3_bucket, &key)
        .await
        .map_err(|e| {
            AppError::InternalError(format!("S3-დან მედიის წაშლა ვერ მოხერხდა: {}", e))
        })?;

    Ok(StatusCode::NO_CONTENT)
}
