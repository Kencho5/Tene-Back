use std::collections::HashMap;

use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{
    error::Result,
    models::{ComboOffer, ComboOfferItem, ComboOfferRequest},
};

pub async fn list_offers(
    pool: &PgPool,
    main_product_id: Option<&str>,
    only_enabled: bool,
) -> Result<Vec<ComboOffer>> {
    let offers = sqlx::query_as::<_, ComboOffer>(
        "SELECT * FROM combo_offers
         WHERE ($1::text IS NULL OR main_product_id = $1)
           AND (NOT $2 OR enabled)
         ORDER BY created_at DESC, id DESC",
    )
    .bind(main_product_id)
    .bind(only_enabled)
    .fetch_all(pool)
    .await?;
    Ok(offers)
}

pub async fn find_offer(pool: &PgPool, id: i32) -> Result<Option<ComboOffer>> {
    let offer = sqlx::query_as::<_, ComboOffer>("SELECT * FROM combo_offers WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(offer)
}

pub async fn find_offers_by_ids(pool: &PgPool, ids: &[i32]) -> Result<HashMap<i32, ComboOffer>> {
    let offers = sqlx::query_as::<_, ComboOffer>("SELECT * FROM combo_offers WHERE id = ANY($1)")
        .bind(ids)
        .fetch_all(pool)
        .await?;
    Ok(offers.into_iter().map(|o| (o.id, o)).collect())
}

pub async fn items_for_offers(
    pool: &PgPool,
    ids: &[i32],
) -> Result<HashMap<i32, Vec<ComboOfferItem>>> {
    let items = sqlx::query_as::<_, ComboOfferItem>(
        "SELECT combo_offer_id, product_id, combo_price, position
         FROM combo_offer_items
         WHERE combo_offer_id = ANY($1)
         ORDER BY combo_offer_id, position, product_id",
    )
    .bind(ids)
    .fetch_all(pool)
    .await?;

    let mut groups: HashMap<i32, Vec<ComboOfferItem>> = HashMap::new();
    for item in items {
        groups.entry(item.combo_offer_id).or_default().push(item);
    }
    Ok(groups)
}

async fn replace_items(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    offer_id: i32,
    req: &ComboOfferRequest,
    keep_combo_price: bool,
) -> Result<()> {
    sqlx::query("DELETE FROM combo_offer_items WHERE combo_offer_id = $1")
        .bind(offer_id)
        .execute(&mut **tx)
        .await?;

    let product_ids: Vec<&str> = req.items.iter().map(|i| i.product_id.as_str()).collect();
    let prices: Vec<Option<Decimal>> = req
        .items
        .iter()
        .map(|i| if keep_combo_price { i.combo_price } else { None })
        .collect();
    let positions: Vec<i32> = (0..req.items.len() as i32).collect();

    sqlx::query(
        "INSERT INTO combo_offer_items (combo_offer_id, product_id, combo_price, position)
         SELECT $1, * FROM UNNEST($2::text[], $3::decimal[], $4::int[])",
    )
    .bind(offer_id)
    .bind(&product_ids)
    .bind(&prices)
    .bind(&positions)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn create_offer(pool: &PgPool, req: &ComboOfferRequest) -> Result<ComboOffer> {
    let mut tx = pool.begin().await?;
    let is_bundle = req.pricing_type == crate::models::ComboPricingType::BundlePrice;

    let offer = sqlx::query_as::<_, ComboOffer>(
        "INSERT INTO combo_offers (main_product_id, title, description, pricing_type, bundle_price, enabled)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(&req.main_product_id)
    .bind(req.title.trim())
    .bind(req.description.as_deref())
    .bind(req.pricing_type.as_str())
    .bind(if is_bundle { req.bundle_price } else { None })
    .bind(req.enabled)
    .fetch_one(&mut *tx)
    .await?;

    replace_items(&mut tx, offer.id, req, !is_bundle).await?;

    tx.commit().await?;
    Ok(offer)
}

pub async fn update_offer(
    pool: &PgPool,
    id: i32,
    req: &ComboOfferRequest,
) -> Result<Option<ComboOffer>> {
    let mut tx = pool.begin().await?;
    let is_bundle = req.pricing_type == crate::models::ComboPricingType::BundlePrice;

    let offer = sqlx::query_as::<_, ComboOffer>(
        "UPDATE combo_offers
         SET main_product_id = $2, title = $3, description = $4, pricing_type = $5,
             bundle_price = $6, enabled = $7, updated_at = NOW()
         WHERE id = $1
         RETURNING *",
    )
    .bind(id)
    .bind(&req.main_product_id)
    .bind(req.title.trim())
    .bind(req.description.as_deref())
    .bind(req.pricing_type.as_str())
    .bind(if is_bundle { req.bundle_price } else { None })
    .bind(req.enabled)
    .fetch_optional(&mut *tx)
    .await?;

    let Some(offer) = offer else {
        return Ok(None);
    };

    replace_items(&mut tx, offer.id, req, !is_bundle).await?;

    tx.commit().await?;
    Ok(Some(offer))
}

pub async fn set_offer_enabled(pool: &PgPool, id: i32, enabled: bool) -> Result<Option<ComboOffer>> {
    let offer = sqlx::query_as::<_, ComboOffer>(
        "UPDATE combo_offers SET enabled = $2, updated_at = NOW() WHERE id = $1 RETURNING *",
    )
    .bind(id)
    .bind(enabled)
    .fetch_optional(pool)
    .await?;
    Ok(offer)
}

pub async fn delete_offer(pool: &PgPool, id: i32) -> Result<u64> {
    let result = sqlx::query("DELETE FROM combo_offers WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
