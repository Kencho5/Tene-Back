use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Todo,
    InProgress,
    Review,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TaskMediaType {
    Image,
    Video,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub state: TaskState,
    pub priority: TaskPriority,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskMedia {
    pub id: i32,
    pub task_id: i32,
    pub media_uuid: Uuid,
    pub media_type: TaskMediaType,
    pub extension: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub state: Option<TaskState>,
    pub priority: Option<TaskPriority>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub state: Option<TaskState>,
    pub priority: Option<TaskPriority>,
}

#[derive(Debug, Deserialize)]
pub struct TaskStateUpdate {
    pub state: TaskState,
}

#[derive(Debug, Deserialize)]
pub struct TaskQuery {
    pub state: Option<TaskState>,
    pub priority: Option<TaskPriority>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TaskWithMedia {
    #[serde(flatten)]
    pub task: Task,
    pub media: Vec<TaskMediaResponse>,
}

#[derive(Debug, Serialize)]
pub struct TaskMediaResponse {
    pub media_uuid: Uuid,
    pub media_type: TaskMediaType,
    pub url: String,
}

#[derive(Debug, Serialize)]
pub struct TaskSearchResponse {
    pub tasks: Vec<TaskWithMedia>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Deserialize)]
pub struct TaskMediaUploadItem {
    pub media_type: TaskMediaType,
    pub content_type: String,
}

#[derive(Debug, Deserialize)]
pub struct TaskMediaUploadRequest {
    pub items: Vec<TaskMediaUploadItem>,
}

#[derive(Debug, Serialize)]
pub struct TaskMediaUploadUrl {
    pub media_uuid: Uuid,
    pub media_type: TaskMediaType,
    pub upload_url: String,
    pub public_url: String,
}

#[derive(Debug, Serialize)]
pub struct TaskMediaUploadResponse {
    pub media: Vec<TaskMediaUploadUrl>,
}
