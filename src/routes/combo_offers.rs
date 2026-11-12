use std::collections::{HashMap, HashSet};

use axum::{
    Json,
    extract::{Path, Query, State},
};
use http::StatusCode;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::{
    AppState,
    error::{AppError, Result},
    models::{
        COMBO_BUNDLE_PRICE, ComboOffer, ComboOfferEnabledRequest, ComboOfferItem,
        ComboOfferItemResponse, ComboOfferQuery, ComboOfferRequest, ComboOfferResponse,
        ComboPricingType, ComboProduct,
    },
    queries::{combo_queries, products_queries},
};

async fn build_responses(
    state: &AppState,
    offers: Vec<ComboOffer>,
    public: bool,
) -> Result<Vec<ComboOfferResponse>> {
    if offers.is_empty() {
        return Ok(Vec::new());
    }

    let offer_ids: Vec<i32> = offers.iter().map(|o| o.id).collect();
    let mut items_map = combo_queries::items_for_offers(&state.db, &offer_ids).await?;

    let mut product_ids: Vec<String> = offers.iter().map(|o| o.main_product_id.clone()).collect();
    product_ids.extend(
        items_map
            .values()
            .flatten()
            .map(|i| i.product_id.clone()),
    );
    product_ids.sort();
    product_ids.dedup();

    let products = products_queries::find_by_ids(&state.db, &product_ids).await?;
    let images = products_queries::find_images_by_product_ids(&state.db, &product_ids).await?;

    let combo_product = |id: &str| -> Option<ComboProduct> {
        products.get(id).map(|p| ComboProduct {
            data: p.clone(),
            images: images.get(id).cloned().unwrap_or_default(),
        })
    };

    let mut out = Vec::with_capacity(offers.len());
    for offer in offers {
        let main_product = combo_product(&offer.main_product_id);
        let rows = items_map.remove(&offer.id).unwrap_or_default();
        let total_rows = rows.len();

        let items: Vec<ComboOfferItemResponse> = rows
            .into_iter()
            .map(|row| ComboOfferItemResponse {
                product: combo_product(&row.product_id),
                product_id: row.product_id,
                combo_price: row.combo_price,
                position: row.position,
            })
            .filter(|item| {
                !public
                    || item
                        .product
                        .as_ref()
                        .is_some_and(|p| p.data.enabled && p.data.cable_type_id.is_none())
            })
            .collect();

        if public {
            let main_ok = main_product.as_ref().is_some_and(|p| p.data.enabled);
            let complete = offer.pricing_type != COMBO_BUNDLE_PRICE || items.len() == total_rows;
            if !main_ok || !complete || items.is_empty() {
                continue;
            }
        }

        out.push(ComboOfferResponse {
            offer,
            main_product,
            items,
        });
    }
    Ok(out)
}

pub async fn get_product_combo_offers(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ComboOfferResponse>>> {
    let offers = combo_queries::list_offers(&state.db, Some(&id), true).await?;
    Ok(Json(build_responses(&state, offers, true).await?))
}

pub async fn list_combo_offers(
    State(state): State<AppState>,
    Query(params): Query<ComboOfferQuery>,
) -> Result<Json<Vec<ComboOfferResponse>>> {
    let offers =
        combo_queries::list_offers(&state.db, params.main_product_id.as_deref(), false).await?;
    Ok(Json(build_responses(&state, offers, false).await?))
}

async fn single_response(state: &AppState, offer: ComboOffer) -> Result<ComboOfferResponse> {
    build_responses(state, vec![offer], false)
        .await?
        .pop()
        .ok_or_else(|| AppError::InternalError("კომბოს ჩატვირთვა ვერ მოხერხდა".to_string()))
}

fn not_found(id: i32) -> AppError {
    AppError::NotFound(format!("კომბო შეთავაზება id-ით {} ვერ მოიძებნა", id))
}

pub async fn get_combo_offer(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<ComboOfferResponse>> {
    let offer = combo_queries::find_offer(&state.db, id)
        .await?
        .ok_or_else(|| not_found(id))?;
    Ok(Json(single_response(&state, offer).await?))
}

async fn validate_request(state: &AppState, req: &mut ComboOfferRequest) -> Result<()> {
    req.title = req.title.trim().to_string();
    if req.title.is_empty() {
        return Err(AppError::BadRequest("სათაური აუცილებელია".to_string()));
    }
    req.description = req
        .description
        .take()
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty());

    if req.items.is_empty() {
        return Err(AppError::BadRequest(
            "მიუთითეთ მინიმუმ ერთი შეთავაზებული პროდუქტი".to_string(),
        ));
    }

    let mut seen = HashSet::new();
    for item in &req.items {
        if item.product_id == req.main_product_id {
            return Err(AppError::BadRequest(
                "ძირითადი პროდუქტი ვერ იქნება შეთავაზებული პროდუქტი".to_string(),
            ));
        }
        if !seen.insert(item.product_id.as_str()) {
            return Err(AppError::BadRequest(format!(
                "დუბლირებული პროდუქტი id-ით {}",
                item.product_id
            )));
        }
    }

    match req.pricing_type {
        ComboPricingType::AddonPrice => {
            for item in &req.items {
                match item.combo_price {
                    Some(p) if p >= Decimal::ZERO => {}
                    _ => {
                        return Err(AppError::BadRequest(format!(
                            "მიუთითეთ კომბო ფასი პროდუქტისთვის {}",
                            item.product_id
                        )));
                    }
                }
            }
        }
        ComboPricingType::BundlePrice => match req.bundle_price {
            Some(p) if p > Decimal::ZERO => {}
            _ => {
                return Err(AppError::BadRequest(
                    "მიუთითეთ კომბოს ფიქსირებული ფასი".to_string(),
                ));
            }
        },
    }

    let mut ids: Vec<String> = req.items.iter().map(|i| i.product_id.clone()).collect();
    ids.push(req.main_product_id.clone());
    let products = products_queries::find_by_ids(&state.db, &ids).await?;
    for id in &ids {
        if !products.contains_key(id) {
            return Err(AppError::NotFound(format!(
                "პროდუქტი id-ით {} ვერ მოიძებნა",
                id
            )));
        }
    }
    for item in &req.items {
        if products[&item.product_id].cable_type_id.is_some() {
            return Err(AppError::BadRequest(format!(
                "კაბელის კონფიგურაციის მქონე პროდუქტი ({}) კომბოში ვერ დაემატება",
                item.product_id
            )));
        }
    }
    if req.pricing_type == ComboPricingType::BundlePrice
        && products[&req.main_product_id].cable_type_id.is_some()
    {
        return Err(AppError::BadRequest(
            "კაბელის კონფიგურაციის მქონე პროდუქტზე ფიქსირებული კომბო ფასი ვერ დაყენდება"
                .to_string(),
        ));
    }

    Ok(())
}

pub async fn create_combo_offer(
    State(state): State<AppState>,
    Json(mut payload): Json<ComboOfferRequest>,
) -> Result<(StatusCode, Json<ComboOfferResponse>)> {
    validate_request(&state, &mut payload).await?;
    let offer = combo_queries::create_offer(&state.db, &payload).await?;
    Ok((StatusCode::CREATED, Json(single_response(&state, offer).await?)))
}

pub async fn update_combo_offer(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(mut payload): Json<ComboOfferRequest>,
) -> Result<Json<ComboOfferResponse>> {
    validate_request(&state, &mut payload).await?;
    let offer = combo_queries::update_offer(&state.db, id, &payload)
        .await?
        .ok_or_else(|| not_found(id))?;
    Ok(Json(single_response(&state, offer).await?))
}

pub async fn set_combo_offer_enabled(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<ComboOfferEnabledRequest>,
) -> Result<Json<ComboOfferResponse>> {
    let offer = combo_queries::set_offer_enabled(&state.db, id, payload.enabled)
        .await?
        .ok_or_else(|| not_found(id))?;
    Ok(Json(single_response(&state, offer).await?))
}

pub async fn delete_combo_offer(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode> {
    if combo_queries::delete_offer(&state.db, id).await? == 0 {
        return Err(not_found(id));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) struct ComboLine {
    pub index: usize,
    pub product_id: String,
    pub quantity: i32,
    pub regular_price: Decimal,
    pub has_cable_config: bool,
}

pub(crate) struct ComboPricing {
    pub index: usize,
    pub price: Decimal,
    pub offer_id: i32,
    pub title: String,
}

const COMBO_UNAVAILABLE: &str =
    "კომბო შეთავაზება აღარ არის ხელმისაწვდომი, გთხოვთ განაახლოთ კალათა";

fn invalid_combo() -> AppError {
    AppError::BadRequest(COMBO_UNAVAILABLE.to_string())
}

pub(crate) fn round_money(value: Decimal) -> Decimal {
    value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

pub(crate) fn allocate_bundle(bundle_price: Decimal, regular_prices: &[Decimal]) -> Vec<Decimal> {
    let n = regular_prices.len();
    if n == 0 {
        return Vec::new();
    }
    let total: Decimal = regular_prices.iter().copied().sum();
    let mut out = Vec::with_capacity(n);
    let mut allocated = Decimal::ZERO;
    for (i, regular) in regular_prices.iter().enumerate() {
        if i == n - 1 {
            out.push((bundle_price - allocated).max(Decimal::ZERO));
            break;
        }
        let share = if total > Decimal::ZERO {
            round_money(bundle_price * *regular / total)
        } else {
            round_money(bundle_price / Decimal::from(n as i64))
        };
        allocated += share;
        out.push(share);
    }
    out
}

pub(crate) fn price_combo_group(
    offer: Option<&ComboOffer>,
    offer_items: &[ComboOfferItem],
    lines: &[ComboLine],
) -> Result<Vec<ComboPricing>> {
    let offer = offer.filter(|o| o.enabled).ok_or_else(invalid_combo)?;

    let mut main_lines = lines.iter().filter(|l| l.product_id == offer.main_product_id);
    let main = main_lines.next().ok_or_else(invalid_combo)?;
    if main_lines.next().is_some() {
        return Err(invalid_combo());
    }

    let mut addons: Vec<(&ComboLine, &ComboOfferItem)> = Vec::new();
    let mut seen = HashSet::new();
    for line in lines.iter().filter(|l| l.index != main.index) {
        let offer_item = offer_items
            .iter()
            .find(|i| i.product_id == line.product_id)
            .ok_or_else(invalid_combo)?;
        if !seen.insert(line.product_id.as_str())
            || line.quantity != main.quantity
            || line.has_cable_config
        {
            return Err(invalid_combo());
        }
        addons.push((line, offer_item));
    }
    if addons.is_empty() {
        return Err(invalid_combo());
    }
    addons.sort_by_key(|(_, item)| (item.position, item.product_id.clone()));

    let mut priced = Vec::with_capacity(lines.len());
    let make = |index: usize, price: Decimal| ComboPricing {
        index,
        price,
        offer_id: offer.id,
        title: offer.title.clone(),
    };

    if offer.pricing_type == COMBO_BUNDLE_PRICE {
        let bundle_price = offer.bundle_price.ok_or_else(invalid_combo)?;
        if addons.len() != offer_items.len() || main.has_cable_config {
            return Err(invalid_combo());
        }
        let mut regulars: Vec<Decimal> = addons.iter().map(|(l, _)| l.regular_price).collect();
        regulars.push(main.regular_price);
        let shares = allocate_bundle(bundle_price, &regulars);
        for ((line, _), share) in addons.iter().zip(shares.iter()) {
            priced.push(make(line.index, *share));
        }
        priced.push(make(main.index, shares[shares.len() - 1]));
    } else {
        for (line, item) in &addons {
            let price = item.combo_price.ok_or_else(invalid_combo)?;
            priced.push(make(line.index, price));
        }
        priced.push(make(main.index, main.regular_price));
    }

    Ok(priced)
}

pub(crate) async fn load_offers(
    state: &AppState,
    offer_ids: &[i32],
) -> Result<(HashMap<i32, ComboOffer>, HashMap<i32, Vec<ComboOfferItem>>)> {
    let offers = combo_queries::find_offers_by_ids(&state.db, offer_ids).await?;
    let items = combo_queries::items_for_offers(&state.db, offer_ids).await?;
    Ok((offers, items))
}
