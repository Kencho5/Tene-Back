use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::Result,
    models::{MoveDirection, Slider, SliderImageVariant, SliderRequest},
};

pub async fn get_sliders(pool: &PgPool) -> Result<Vec<Slider>> {
    let sliders = sqlx::query_as::<_, Slider>(
        "SELECT * FROM home_sliders ORDER BY display_order ASC, id ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(sliders)
}

pub async fn get_public_sliders(pool: &PgPool) -> Result<Vec<Slider>> {
    let sliders = sqlx::query_as::<_, Slider>(
        "SELECT * FROM home_sliders
         WHERE enabled AND (desktop_image_uuid IS NOT NULL OR mobile_image_uuid IS NOT NULL)
         ORDER BY display_order ASC, id ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(sliders)
}

pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Slider>> {
    let slider = sqlx::query_as::<_, Slider>("SELECT * FROM home_sliders WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(slider)
}

pub async fn create_slider(pool: &PgPool, req: &SliderRequest) -> Result<Slider> {
    let slider = sqlx::query_as::<_, Slider>(
        "INSERT INTO home_sliders (title, link_url, enabled, display_order)
         VALUES ($1, $2, COALESCE($3, TRUE), (SELECT COALESCE(MAX(display_order) + 1, 0) FROM home_sliders))
         RETURNING *",
    )
    .bind(&req.title)
    .bind(&req.link_url)
    .bind(req.enabled)
    .fetch_one(pool)
    .await?;

    Ok(slider)
}

pub async fn update_slider(pool: &PgPool, id: i32, req: &SliderRequest) -> Result<Option<Slider>> {
    let slider = sqlx::query_as::<_, Slider>(
        "UPDATE home_sliders
         SET title = $2, link_url = $3, enabled = COALESCE($4, enabled), updated_at = NOW()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&req.title)
    .bind(&req.link_url)
    .bind(req.enabled)
    .fetch_optional(pool)
    .await?;

    Ok(slider)
}

pub async fn delete_slider(pool: &PgPool, id: i32) -> Result<bool> {
    let result = sqlx::query("DELETE FROM home_sliders WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn set_slider_image(
    pool: &PgPool,
    id: i32,
    variant: SliderImageVariant,
    image: Option<(Uuid, &str)>,
) -> Result<()> {
    let sql = match variant {
        SliderImageVariant::Desktop => {
            "UPDATE home_sliders
             SET desktop_image_uuid = $2, desktop_image_extension = $3, updated_at = NOW()
             WHERE id = $1"
        }
        SliderImageVariant::Mobile => {
            "UPDATE home_sliders
             SET mobile_image_uuid = $2, mobile_image_extension = $3, updated_at = NOW()
             WHERE id = $1"
        }
    };

    sqlx::query(sql)
        .bind(id)
        .bind(image.map(|(uuid, _)| uuid))
        .bind(image.map(|(_, ext)| ext))
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn move_slider(pool: &PgPool, id: i32, direction: MoveDirection) -> Result<bool> {
    let mut tx = pool.begin().await?;

    let mut ids: Vec<i32> = sqlx::query_scalar(
        "SELECT id FROM home_sliders ORDER BY display_order ASC, id ASC FOR UPDATE",
    )
    .fetch_all(&mut *tx)
    .await?;

    let Some(index) = ids.iter().position(|&slider_id| slider_id == id) else {
        return Ok(false);
    };

    let target = match direction {
        MoveDirection::Up => index.checked_sub(1),
        MoveDirection::Down => Some(index + 1).filter(|&i| i < ids.len()),
    };

    if let Some(target) = target {
        ids.swap(index, target);
    }

    let orders: Vec<i32> = (0..ids.len() as i32).collect();

    sqlx::query(
        "UPDATE home_sliders s
         SET display_order = v.display_order, updated_at = NOW()
         FROM UNNEST($1::int[], $2::int[]) AS v(id, display_order)
         WHERE s.id = v.id AND s.display_order <> v.display_order",
    )
    .bind(&ids)
    .bind(&orders)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(true)
}
