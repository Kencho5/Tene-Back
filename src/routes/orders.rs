use std::collections::{HashMap, HashSet};

use axum::{Extension, Json, extract::State, http::StatusCode, response::IntoResponse};
use rust_decimal::{Decimal, dec, prelude::ToPrimitive};
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{CheckoutRequest, CheckoutResponse, OrderItemData, OrderResponse},
    queries::{order_queries, products_queries},
    services::flitt_service,
    utils::extractors::extract_user_id,
    utils::jwt::Claims,
};

const DELIVERY_PRICE: Decimal = dec!(5);

pub async fn checkout(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CheckoutRequest>,
) -> Result<Json<CheckoutResponse>> {
    let user_id = extract_user_id(&claims)?;

    if payload.items.is_empty() {
        return Err(AppError::BadRequest("კალათა ცარიელია".to_string()));
    }

    if payload.email.is_empty() || !payload.email.contains('@') {
        return Err(AppError::BadRequest("არასწორი ელფოსტა".to_string()));
    }

    if payload.address.is_empty() {
        return Err(AppError::BadRequest("მისამართი აუცილებელია".to_string()));
    }

    for item in &payload.items {
        if item.quantity <= 0 {
            return Err(AppError::BadRequest(format!(
                "არასწორი რაოდენობა პროდუქტისთვის {}",
                item.product_id
            )));
        }
    }

    // Aggregate total demand per product+color for accurate stock checks
    let mut demand: HashMap<(i32, Option<&str>), i32> = HashMap::new();
    for item in &payload.items {
        *demand
            .entry((item.product_id, item.color.as_deref()))
            .or_insert(0) += item.quantity;
    }

    // Batch-fetch all products and images
    let requested_ids: Vec<i32> = payload.items.iter().map(|i| i.product_id).collect();
    let all_products = products_queries::find_by_ids(&state.db, &requested_ids).await?;
    let all_images =
        products_queries::find_images_by_product_ids(&state.db, &requested_ids).await?;

    let mut total_amount = Decimal::ZERO;
    let mut order_items = Vec::with_capacity(payload.items.len());

    for item in &payload.items {
        let product = all_products
            .get(&item.product_id)
            .ok_or_else(|| AppError::NotFound(format!("პროდუქტი {} ვერ მოიძებნა", item.product_id)))?;

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

        // Require color when product has multiple color variants
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

        // Find the matching image for display
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

        // Check stock against total demand for this product+color
        let total_demand = demand[&(item.product_id, item.color.as_deref())];
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

        let price = if product.discount > Decimal::ZERO {
            product.price * (Decimal::ONE - product.discount / Decimal::from(100))
        } else {
            product.price
        };

        total_amount += price * Decimal::from(item.quantity);

        order_items.push(OrderItemData {
            product_id: item.product_id,
            color: item.color.clone(),
            quantity: item.quantity,
            price,
            product_name: product.name.clone(),
            image: serde_json::to_value(image)
                .map_err(|e| AppError::InternalError(e.to_string()))?,
        });
    }

    let amount_tetri = ((total_amount + DELIVERY_PRICE) * Decimal::from(100))
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
    let response_url = format!("{}/checkout/result", state.frontend_url);

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
            tracing::warn!("Flitt callback: order {} not found or already processed", order_id);
            StatusCode::OK
        }
        Err(e) => {
            tracing::error!("Failed to update order status: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

pub async fn get_orders(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<OrderResponse>>> {
    let user_id = extract_user_id(&claims)?;
    let orders = order_queries::get_user_orders(&state.db, user_id).await?;

    let order_db_ids: Vec<i32> = orders.iter().map(|o| o.id).collect();
    let all_items = order_queries::get_items_for_orders(&state.db, &order_db_ids).await?;

    let mut items_map: std::collections::HashMap<i32, Vec<_>> = std::collections::HashMap::new();
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
