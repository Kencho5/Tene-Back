use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::Result,
    models::{
        CreateTaskRequest, Task, TaskMedia, TaskMediaType, TaskQuery, TaskState, UpdateTaskRequest,
    },
};

const DEFAULT_PAGE_SIZE: i64 = 20;
const MAX_PAGE_SIZE: i64 = 100;

pub async fn create_task(pool: &PgPool, req: &CreateTaskRequest) -> Result<Task> {
    let task = sqlx::query_as::<_, Task>(
        r#"
        INSERT INTO tasks (title, description, state, priority)
        VALUES ($1, $2, COALESCE($3, 'todo'), COALESCE($4, 'medium'))
        RETURNING *
        "#,
    )
    .bind(&req.title)
    .bind(&req.description)
    .bind(&req.state)
    .bind(&req.priority)
    .fetch_one(pool)
    .await?;

    Ok(task)
}

pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Task>> {
    let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(task)
}

pub async fn update_task(pool: &PgPool, id: i32, req: &UpdateTaskRequest) -> Result<Task> {
    let task = sqlx::query_as::<_, Task>(
        r#"
        UPDATE tasks
        SET
            title       = COALESCE($1, title),
            description = COALESCE($2, description),
            state       = COALESCE($3, state),
            priority    = COALESCE($4, priority),
            updated_at  = NOW()
        WHERE id = $5
        RETURNING *
        "#,
    )
    .bind(&req.title)
    .bind(&req.description)
    .bind(&req.state)
    .bind(&req.priority)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(task)
}

pub async fn update_task_state(pool: &PgPool, id: i32, state: &TaskState) -> Result<Task> {
    let task = sqlx::query_as::<_, Task>(
        "UPDATE tasks SET state = $1, updated_at = NOW() WHERE id = $2 RETURNING *",
    )
    .bind(state)
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok(task)
}

pub async fn delete_task(pool: &PgPool, id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn search_tasks(pool: &PgPool, params: TaskQuery) -> Result<(Vec<Task>, i64, i64, i64)> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0);

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT *, COUNT(*) OVER() as total_count FROM tasks WHERE 1=1",
    );

    if let Some(state) = &params.state {
        qb.push(" AND state = ");
        qb.push_bind(state);
    }
    if let Some(priority) = &params.priority {
        qb.push(" AND priority = ");
        qb.push_bind(priority);
    }

    qb.push(" ORDER BY created_at DESC LIMIT ");
    qb.push_bind(limit);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        task: Task,
        total_count: i64,
    }

    let rows = qb.build_query_as::<Row>().fetch_all(pool).await?;
    let total = rows.first().map(|r| r.total_count).unwrap_or(0);
    let tasks = rows.into_iter().map(|r| r.task).collect();

    Ok((tasks, total, limit, offset))
}

pub async fn add_task_media(
    pool: &PgPool,
    task_id: i32,
    media_uuid: Uuid,
    media_type: &TaskMediaType,
    extension: &str,
) -> Result<TaskMedia> {
    let media = sqlx::query_as::<_, TaskMedia>(
        r#"
        INSERT INTO task_media (task_id, media_uuid, media_type, extension)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
    )
    .bind(task_id)
    .bind(media_uuid)
    .bind(media_type)
    .bind(extension)
    .fetch_one(pool)
    .await?;
    Ok(media)
}

pub async fn get_task_media(pool: &PgPool, task_id: i32) -> Result<Vec<TaskMedia>> {
    let media = sqlx::query_as::<_, TaskMedia>(
        "SELECT * FROM task_media WHERE task_id = $1 ORDER BY created_at ASC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?;
    Ok(media)
}

pub async fn get_media_for_tasks(pool: &PgPool, task_ids: &[i32]) -> Result<Vec<TaskMedia>> {
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }
    let media = sqlx::query_as::<_, TaskMedia>(
        "SELECT * FROM task_media WHERE task_id = ANY($1) ORDER BY created_at ASC",
    )
    .bind(task_ids)
    .fetch_all(pool)
    .await?;
    Ok(media)
}

pub async fn delete_task_media(
    pool: &PgPool,
    task_id: i32,
    media_uuid: Uuid,
) -> Result<Option<TaskMedia>> {
    let deleted = sqlx::query_as::<_, TaskMedia>(
        "DELETE FROM task_media WHERE task_id = $1 AND media_uuid = $2 RETURNING *",
    )
    .bind(task_id)
    .bind(media_uuid)
    .fetch_optional(pool)
    .await?;
    Ok(deleted)
}
