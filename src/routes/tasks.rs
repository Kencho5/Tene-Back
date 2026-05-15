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
        CreateTaskRequest, Task, TaskMediaResponse, TaskMediaType, TaskMediaUploadRequest,
        TaskMediaUploadResponse, TaskMediaUploadUrl, TaskQuery, TaskSearchResponse,
        TaskStateUpdate, TaskWithMedia, UpdateTaskRequest,
    },
    queries::task_queries,
    services::image_url_service::{delete_objects_by_prefix, delete_single_object, put_object_url},
};

fn env_prefix(state: &AppState) -> &'static str {
    match state.environment {
        crate::config::Environment::Staging => "tasks-staging",
        crate::config::Environment::Main => "tasks-main",
    }
}

fn ext_for(media_type: &TaskMediaType, content_type: &str) -> &'static str {
    match media_type {
        TaskMediaType::Image => match content_type {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            "image/gif" => "gif",
            _ => "jpg",
        },
        TaskMediaType::Video => match content_type {
            "video/mp4" => "mp4",
            "video/webm" => "webm",
            "video/quicktime" => "mov",
            _ => "mp4",
        },
        TaskMediaType::Audio => match content_type {
            "audio/mpeg" => "mp3",
            "audio/mp4" | "audio/m4a" => "m4a",
            "audio/webm" => "webm",
            "audio/ogg" => "ogg",
            "audio/wav" | "audio/wave" => "wav",
            _ => "mp3",
        },
    }
}

fn media_url(state: &AppState, task_id: i32, m: &crate::models::TaskMedia) -> String {
    format!(
        "{}/{}/{}/{}.{}",
        state.assets_url,
        env_prefix(state),
        task_id,
        m.media_uuid,
        m.extension
    )
}

async fn load_task_with_media(state: &AppState, task: Task) -> Result<TaskWithMedia> {
    let media = task_queries::get_task_media(&state.db, task.id).await?;
    let media: Vec<TaskMediaResponse> = media
        .into_iter()
        .map(|m| TaskMediaResponse {
            url: media_url(state, task.id, &m),
            media_uuid: m.media_uuid,
            media_type: m.media_type,
        })
        .collect();
    Ok(TaskWithMedia { task, media })
}

pub async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<Json<TaskWithMedia>> {
    if payload.title.trim().is_empty() {
        return Err(AppError::BadRequest("title აუცილებელია".to_string()));
    }
    let task = task_queries::create_task(&state.db, &payload).await?;
    let resp = load_task_with_media(&state, task).await?;
    Ok(Json(resp))
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<TaskWithMedia>> {
    let task = task_queries::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("task id-ით {} ვერ მოიძებნა", id)))?;
    let resp = load_task_with_media(&state, task).await?;
    Ok(Json(resp))
}

pub async fn search_tasks(
    State(state): State<AppState>,
    Query(params): Query<TaskQuery>,
) -> Result<Json<TaskSearchResponse>> {
    let (tasks, total, limit, offset) = task_queries::search_tasks(&state.db, params).await?;

    let ids: Vec<i32> = tasks.iter().map(|t| t.id).collect();
    let media_rows = task_queries::get_media_for_tasks(&state.db, &ids).await?;

    let mut map: HashMap<i32, Vec<TaskMediaResponse>> = HashMap::new();
    for m in media_rows {
        let url = media_url(&state, m.task_id, &m);
        map.entry(m.task_id).or_default().push(TaskMediaResponse {
            url,
            media_uuid: m.media_uuid,
            media_type: m.media_type,
        });
    }

    let tasks = tasks
        .into_iter()
        .map(|t| TaskWithMedia {
            media: map.remove(&t.id).unwrap_or_default(),
            task: t,
        })
        .collect();

    Ok(Json(TaskSearchResponse {
        tasks,
        total,
        limit,
        offset,
    }))
}

pub async fn update_task(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<UpdateTaskRequest>,
) -> Result<Json<TaskWithMedia>> {
    if task_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!("task id-ით {} ვერ მოიძებნა", id)));
    }
    let task = task_queries::update_task(&state.db, id, &payload).await?;
    let resp = load_task_with_media(&state, task).await?;
    Ok(Json(resp))
}

pub async fn update_task_state(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<TaskStateUpdate>,
) -> Result<Json<TaskWithMedia>> {
    if task_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!("task id-ით {} ვერ მოიძებნა", id)));
    }
    let task = task_queries::update_task_state(&state.db, id, &payload.state).await?;
    let resp = load_task_with_media(&state, task).await?;
    Ok(Json(resp))
}

pub async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode> {
    if task_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!("task id-ით {} ვერ მოიძებნა", id)));
    }

    let prefix = format!("{}/{}/", env_prefix(&state), id);
    delete_objects_by_prefix(&state.s3_client, &state.s3_bucket, &prefix)
        .await
        .map_err(|e| {
            AppError::InternalError(format!("S3-დან მედიის წაშლა ვერ მოხერხდა: {}", e))
        })?;

    task_queries::delete_task(&state.db, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn generate_task_media_urls(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<TaskMediaUploadRequest>,
) -> Result<Json<TaskMediaUploadResponse>> {
    if task_queries::find_by_id(&state.db, id).await?.is_none() {
        return Err(AppError::NotFound(format!("task id-ით {} ვერ მოიძებნა", id)));
    }

    let mut out = Vec::with_capacity(payload.items.len());

    for item in payload.items {
        let media_uuid = Uuid::new_v4();
        let extension = ext_for(&item.media_type, &item.content_type);
        let key = format!(
            "{}/{}/{}.{}",
            env_prefix(&state),
            id,
            media_uuid,
            extension
        );

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

        task_queries::add_task_media(&state.db, id, media_uuid, &item.media_type, extension)
            .await?;

        out.push(TaskMediaUploadUrl {
            media_uuid,
            media_type: item.media_type,
            upload_url,
            public_url,
        });
    }

    Ok(Json(TaskMediaUploadResponse { media: out }))
}

pub async fn delete_task_media(
    State(state): State<AppState>,
    Path((task_id, media_uuid)): Path<(i32, Uuid)>,
) -> Result<StatusCode> {
    let deleted = task_queries::delete_task_media(&state.db, task_id, media_uuid)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "მედია {} ვერ მოიძებნა task {}-ისთვის",
                media_uuid, task_id
            ))
        })?;

    let key = format!(
        "{}/{}/{}.{}",
        env_prefix(&state),
        task_id,
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
