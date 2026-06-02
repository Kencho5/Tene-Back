use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::Result,
    models::{
        Blog, BlogMedia, BlogMediaType, BlogQuery, CreateBlogRequest, UpdateBlogRequest,
    },
};

const DEFAULT_PAGE_SIZE: i64 = 20;
const MAX_PAGE_SIZE: i64 = 100;

pub async fn create_blog(pool: &PgPool, req: &CreateBlogRequest, slug: &str) -> Result<Blog> {
    let blog = sqlx::query_as::<_, Blog>(
        r#"
        INSERT INTO blogs (title, slug, excerpt, content, status, published_at)
        VALUES (
            $1, $2, $3, $4,
            COALESCE($5, 'draft'),
            CASE WHEN COALESCE($5, 'draft') = 'published' THEN NOW() END
        )
        RETURNING *
        "#,
    )
    .bind(&req.title)
    .bind(slug)
    .bind(&req.excerpt)
    .bind(&req.content)
    .bind(&req.status)
    .fetch_one(pool)
    .await?;

    Ok(blog)
}

pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Blog>> {
    let blog = sqlx::query_as::<_, Blog>("SELECT * FROM blogs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(blog)
}

pub async fn find_published_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Blog>> {
    let blog = sqlx::query_as::<_, Blog>(
        "SELECT * FROM blogs WHERE slug = $1 AND status = 'published'",
    )
    .bind(slug)
    .fetch_optional(pool)
    .await?;
    Ok(blog)
}

pub async fn list_published(
    pool: &PgPool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<Blog>, i64, i64, i64)> {
    let limit = limit.clamp(1, MAX_PAGE_SIZE);
    let offset = offset.max(0);

    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        blog: Blog,
        total_count: i64,
    }

    let rows = sqlx::query_as::<_, Row>(
        r#"
        SELECT *, COUNT(*) OVER() as total_count
        FROM blogs
        WHERE status = 'published'
        ORDER BY published_at DESC NULLS LAST, created_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let total = rows.first().map(|r| r.total_count).unwrap_or(0);
    let blogs = rows.into_iter().map(|r| r.blog).collect();

    Ok((blogs, total, limit, offset))
}

pub async fn slug_exists(pool: &PgPool, slug: &str, exclude_id: Option<i32>) -> Result<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM blogs WHERE slug = $1 AND ($2::int IS NULL OR id <> $2))",
    )
    .bind(slug)
    .bind(exclude_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

pub async fn update_blog(
    pool: &PgPool,
    id: i32,
    req: &UpdateBlogRequest,
    slug: Option<&str>,
) -> Result<Blog> {
    let blog = sqlx::query_as::<_, Blog>(
        r#"
        UPDATE blogs
        SET
            title        = COALESCE($1, title),
            slug         = COALESCE($2, slug),
            excerpt      = COALESCE($3, excerpt),
            content      = COALESCE($4, content),
            status       = COALESCE($5, status),
            published_at = CASE
                WHEN COALESCE($5, status) = 'published' AND published_at IS NULL THEN NOW()
                ELSE published_at
            END,
            updated_at   = NOW()
        WHERE id = $6
        RETURNING *
        "#,
    )
    .bind(&req.title)
    .bind(slug)
    .bind(&req.excerpt)
    .bind(&req.content)
    .bind(&req.status)
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(blog)
}

pub async fn delete_blog(pool: &PgPool, id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM blogs WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn search_blogs(pool: &PgPool, params: BlogQuery) -> Result<(Vec<Blog>, i64, i64, i64)> {
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
    let offset = params.offset.unwrap_or(0);

    let mut qb = sqlx::QueryBuilder::<sqlx::Postgres>::new(
        "SELECT *, COUNT(*) OVER() as total_count FROM blogs WHERE 1=1",
    );

    if let Some(status) = &params.status {
        qb.push(" AND status = ");
        qb.push_bind(status);
    }

    qb.push(" ORDER BY created_at DESC LIMIT ");
    qb.push_bind(limit);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        blog: Blog,
        total_count: i64,
    }

    let rows = qb.build_query_as::<Row>().fetch_all(pool).await?;
    let total = rows.first().map(|r| r.total_count).unwrap_or(0);
    let blogs = rows.into_iter().map(|r| r.blog).collect();

    Ok((blogs, total, limit, offset))
}

pub async fn add_blog_media(
    pool: &PgPool,
    blog_id: i32,
    media_uuid: Uuid,
    media_type: &BlogMediaType,
    extension: &str,
    is_thumbnail: bool,
) -> Result<BlogMedia> {
    let mut tx = pool.begin().await?;

    if is_thumbnail {
        sqlx::query("UPDATE blog_media SET is_thumbnail = FALSE WHERE blog_id = $1 AND is_thumbnail")
            .bind(blog_id)
            .execute(&mut *tx)
            .await?;
    }

    let media = sqlx::query_as::<_, BlogMedia>(
        r#"
        INSERT INTO blog_media (blog_id, media_uuid, media_type, extension, is_thumbnail)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
    )
    .bind(blog_id)
    .bind(media_uuid)
    .bind(media_type)
    .bind(extension)
    .bind(is_thumbnail)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(media)
}

pub async fn set_blog_media_thumbnail(
    pool: &PgPool,
    blog_id: i32,
    media_uuid: Uuid,
    is_thumbnail: bool,
) -> Result<Option<BlogMedia>> {
    let mut tx = pool.begin().await?;

    if is_thumbnail {
        sqlx::query(
            "UPDATE blog_media SET is_thumbnail = FALSE WHERE blog_id = $1 AND is_thumbnail AND media_uuid <> $2",
        )
        .bind(blog_id)
        .bind(media_uuid)
        .execute(&mut *tx)
        .await?;
    }

    let updated = sqlx::query_as::<_, BlogMedia>(
        "UPDATE blog_media SET is_thumbnail = $3 WHERE blog_id = $1 AND media_uuid = $2 RETURNING *",
    )
    .bind(blog_id)
    .bind(media_uuid)
    .bind(is_thumbnail)
    .fetch_optional(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(updated)
}

pub async fn get_blog_media(pool: &PgPool, blog_id: i32) -> Result<Vec<BlogMedia>> {
    let media = sqlx::query_as::<_, BlogMedia>(
        "SELECT * FROM blog_media WHERE blog_id = $1 ORDER BY created_at ASC",
    )
    .bind(blog_id)
    .fetch_all(pool)
    .await?;
    Ok(media)
}

pub async fn get_media_for_blogs(pool: &PgPool, blog_ids: &[i32]) -> Result<Vec<BlogMedia>> {
    if blog_ids.is_empty() {
        return Ok(Vec::new());
    }
    let media = sqlx::query_as::<_, BlogMedia>(
        "SELECT * FROM blog_media WHERE blog_id = ANY($1) ORDER BY created_at ASC",
    )
    .bind(blog_ids)
    .fetch_all(pool)
    .await?;
    Ok(media)
}

pub async fn delete_blog_media(
    pool: &PgPool,
    blog_id: i32,
    media_uuid: Uuid,
) -> Result<Option<BlogMedia>> {
    let deleted = sqlx::query_as::<_, BlogMedia>(
        "DELETE FROM blog_media WHERE blog_id = $1 AND media_uuid = $2 RETURNING *",
    )
    .bind(blog_id)
    .bind(media_uuid)
    .fetch_optional(pool)
    .await?;
    Ok(deleted)
}
