use std::collections::{HashMap, HashSet};

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde_json::json;
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{CartItem, CheckoutRequest, CheckoutResponse, OrderItemData, OrderResponse},
    queries::{order_queries, products_queries},
    services::{delivery_service, flitt_service},
    utils::extractors::{OptionalClaims, extract_user_id},
    utils::jwt::Claims,
};

const CABLE_PRICES: &[(i32, i32, i32)] = &[
    // (length_cm, price_67w, price_120w)
    (20, 15, 25),
    (50, 20, 35),
    (70, 22, 40),
    (100, 25, 50),
    (130, 30, 55),
    (150, 35, 60),
    (200, 37, 65),
    (250, 40, 70),
    (300, 45, 75),
    (350, 50, 80),
    (400, 55, 85),
    (450, 60, 90),
    (500, 65, 95),
];

fn cable_price(watts: i32, length_cm: i32) -> Option<i32> {
    let row = CABLE_PRICES.iter().find(|(l, _, _)| *l == length_cm)?;
    match watts {
        67 => Some(row.1),
        120 => Some(row.2),
        _ => None,
    }
}

pub async fn checkout(
    State(state): State<AppState>,
    OptionalClaims(claims): OptionalClaims,
    Json(payload): Json<CheckoutRequest>,
) -> Result<Json<CheckoutResponse>> {
    let user_id = claims.as_ref().and_then(|c| extract_user_id(c).ok());

    validate_checkout_request(&payload)?;

    let (order_items, subtotal) = build_order_items(&state, &payload).await?;

    let delivery = delivery_service::calculate_delivery(
        &payload.delivery_type,
        &payload.delivery_time,
        payload.city.as_deref(),
        subtotal,
    )?;

    let amount_tetri = ((subtotal + delivery) * Decimal::from(100))
        .trunc()
        .to_i32()
        .ok_or_else(|| AppError::InternalError("თანხის გამოთვლა ვერ მოხერხდა".to_string()))?;

    if amount_tetri <= 0 {
        return Err(AppError::BadRequest(
            "შეკვეთის თანხა უნდა იყოს დადებითი".to_string(),
        ));
    }

    let order_id = format!("tene_{}", Uuid::new_v4());

    order_queries::create_order_with_items(
        &state.db,
        user_id,
        &order_id,
        amount_tetri,
        &payload,
        &order_items,
    )
    .await?;

    let server_callback_url = format!("{}/payments/callback", state.backend_url);
    let response_url = format!("{}/payments/redirect", state.backend_url);

    let checkout_url = flitt_service::create_checkout_url(
        state.flitt_merchant_id,
        &state.flitt_secret_key,
        &order_id,
        amount_tetri,
        &format!("Tene order {}", order_id),
        &server_callback_url,
        &response_url,
    )
    .await?;

    order_queries::update_order_checkout_url(&state.db, &order_id, &checkout_url).await?;

    Ok(Json(CheckoutResponse {
        order_id,
        checkout_url,
    }))
}

fn validate_checkout_request(payload: &CheckoutRequest) -> Result<()> {
    if payload.items.is_empty() {
        return Err(AppError::BadRequest("კალათა ცარიელია".to_string()));
    }

    if payload.email.is_empty() || !payload.email.contains('@') {
        return Err(AppError::BadRequest("არასწორი ელფოსტა".to_string()));
    }

    if payload.delivery_type != "pickup" {
        if payload.address.is_empty() {
            return Err(AppError::BadRequest("მისამართი აუცილებელია".to_string()));
        }
        if payload.city.as_deref().unwrap_or("").is_empty() {
            return Err(AppError::BadRequest("ქალაქი აუცილებელია".to_string()));
        }
    }

    for item in &payload.items {
        if item.quantity <= 0 {
            return Err(AppError::BadRequest(format!(
                "არასწორი რაოდენობა პროდუქტისთვის {}",
                item.product_id
            )));
        }
        if let Some(cfg) = &item.cable_config {
            if cable_price(cfg.watts, cfg.length_cm).is_none() {
                return Err(AppError::BadRequest(format!(
                    "არასწორი კაბელის კონფიგურაცია პროდუქტისთვის {}",
                    item.product_id
                )));
            }
        }
    }

    Ok(())
}

async fn build_order_items(
    state: &AppState,
    payload: &CheckoutRequest,
) -> Result<(Vec<OrderItemData>, Decimal)> {
    let mut demand: HashMap<(&str, Option<&str>), i32> = HashMap::new();
    for item in &payload.items {
        *demand
            .entry((item.product_id.as_str(), item.color.as_deref()))
            .or_insert(0) += item.quantity;
    }

    let requested_ids: Vec<String> = payload.items.iter().map(|i| i.product_id.clone()).collect();
    let all_products = products_queries::find_by_ids(&state.db, &requested_ids).await?;
    let all_images =
        products_queries::find_images_by_product_ids(&state.db, &requested_ids).await?;

    let mut subtotal = Decimal::ZERO;
    let mut order_items = Vec::with_capacity(payload.items.len());

    for item in &payload.items {
        let product = all_products.get(&item.product_id).ok_or_else(|| {
            AppError::NotFound(format!("პროდუქტი {} ვერ მოიძებნა", item.product_id))
        })?;

        if !product.enabled {
            return Err(AppError::BadRequest(format!(
                "პროდუქტი {} მიუწვდომელია",
                item.product_id
            )));
        }

        let images = all_images
            .get(&item.product_id)
            .map(|v| v.as_slice())
            .unwrap_or_default();

        let distinct_colors: HashSet<_> = images
            .iter()
            .filter_map(|img| img.color.as_deref())
            .collect();

        if distinct_colors.len() > 1 && item.color.is_none() {
            return Err(AppError::BadRequest(format!(
                "ფერი აუცილებელია პროდუქტისთვის {}",
                item.product_id
            )));
        }

        let image = match &item.color {
            Some(color) => images
                .iter()
                .find(|img| img.color.as_deref() == Some(color.as_str())),
            None => images.iter().find(|img| img.is_primary).or(images.first()),
        }
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "ფერი მიუწვდომელია პროდუქტისთვის {}",
                item.product_id
            ))
        })?;

        let total_demand = demand[&(item.product_id.as_str(), item.color.as_deref())];
        let stock: i32 = match &item.color {
            Some(color) => images
                .iter()
                .filter(|img| img.color.as_deref() == Some(color.as_str()))
                .map(|img| img.quantity)
                .sum(),
            None => image.quantity,
        };

        if stock < total_demand {
            return Err(AppError::BadRequest(format!(
                "არასაკმარისი მარაგი პროდუქტისთვის {}",
                item.product_id
            )));
        }

        let price = item_price(product.price, product.discount, item);
        subtotal += price * Decimal::from(item.quantity);

        let cable_config_json = item
            .cable_config
            .as_ref()
            .map(|c| json!({ "watts": c.watts, "length_cm": c.length_cm }));

        order_items.push(OrderItemData {
            product_id: item.product_id.clone(),
            color: item.color.clone(),
            quantity: item.quantity,
            price,
            product_name: product.name.clone(),
            image: serde_json::to_value(image)?,
            cable_config: cable_config_json,
        });
    }

    Ok((order_items, subtotal))
}

fn item_price(base_price: Decimal, discount: Decimal, item: &CartItem) -> Decimal {
    if let Some(cfg) = &item.cable_config {
        // Validated upstream — safe to expect
        return Decimal::from(
            cable_price(cfg.watts, cfg.length_cm).expect("cable_price validated"),
        );
    }
    if discount > Decimal::ZERO {
        return base_price * (Decimal::ONE - discount / Decimal::from(100));
    }
    base_price
}

pub async fn flitt_callback(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    if !flitt_service::verify_callback_signature(&state.flitt_secret_key, &payload) {
        tracing::warn!("Invalid Flitt callback signature");
        return StatusCode::BAD_REQUEST;
    }

    let order_id = match payload.get("order_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => {
            tracing::warn!("Flitt callback missing order_id");
            return StatusCode::BAD_REQUEST;
        }
    };

    let order_status = payload
        .get("order_status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let payment_id = payload
        .get("payment_id")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    tracing::info!(
        "Flitt callback: order_id={}, status={}, payment_id={:?}",
        order_id,
        order_status,
        payment_id
    );

    match order_queries::update_order_status_and_deduct_stock(
        &state.db,
        order_id,
        order_status,
        payment_id,
    )
    .await
    {
        Ok(Some((_order, stock_ok))) => {
            if !stock_ok {
                tracing::warn!("Insufficient stock for approved order {}", order_id);
            }
            StatusCode::OK
        }
        Ok(None) => {
            tracing::warn!(
                "Flitt callback: order {} not found or already processed",
                order_id
            );
            StatusCode::OK
        }
        Err(e) => {
            tracing::error!("Failed to update order status: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn payment_redirect(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Redirect {
    let order_id = params.get("order_id").cloned().unwrap_or_default();
    let order_status = params.get("order_status").cloned().unwrap_or_default();
    let target = format!(
        "{}/checkout/result?order_id={}&order_status={}",
        state.frontend_url, order_id, order_status
    );
    Redirect::to(&target)
}

pub async fn get_order(
    State(state): State<AppState>,
    OptionalClaims(claims): OptionalClaims,
    Path(order_id): Path<String>,
) -> Result<Json<OrderResponse>> {
    let order = order_queries::get_order_by_order_id(&state.db, &order_id)
        .await?
        .ok_or_else(|| AppError::NotFound("შეკვეთა ვერ მოიძებნა".to_string()))?;

    if let Some(owner_id) = order.user_id {
        let viewer_id = claims
            .as_ref()
            .and_then(|c| extract_user_id(c).ok())
            .ok_or_else(|| AppError::Unauthorized("არაავტორიზებული".to_string()))?;
        if viewer_id != owner_id {
            return Err(AppError::NotFound("შეკვეთა ვერ მოიძებნა".to_string()));
        }
    }

    let items = order_queries::get_items_for_orders(&state.db, &[order.id]).await?;

    Ok(Json(OrderResponse { order, items }))
}

pub async fn get_orders(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<OrderResponse>>> {
    let user_id = extract_user_id(&claims)?;
    let orders = order_queries::get_user_orders(&state.db, user_id).await?;

    let order_db_ids: Vec<i32> = orders.iter().map(|o| o.id).collect();
    let all_items = order_queries::get_items_for_orders(&state.db, &order_db_ids).await?;

    let mut items_map: HashMap<i32, Vec<_>> = HashMap::new();
    for item in all_items {
        items_map.entry(item.order_id).or_default().push(item);
    }

    let response = orders
        .into_iter()
        .map(|order| {
            let items = items_map.remove(&order.id).unwrap_or_default();
            OrderResponse { order, items }
        })
        .collect();

    Ok(Json(response))
}
