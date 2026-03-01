use axum::{Extension, Json, extract::State, http::StatusCode, response::IntoResponse};
use rust_decimal::{Decimal, dec};
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{CheckoutRequest, CheckoutResponse, OrderResponse},
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
        return Err(AppError::BadRequest("Cart is empty".to_string()));
    }

    if payload.email.is_empty() || !payload.email.contains('@') {
        return Err(AppError::BadRequest("Invalid email".to_string()));
    }

    if payload.address.is_empty() {
        return Err(AppError::BadRequest("Address is required".to_string()));
    }

    let mut total_amount = Decimal::ZERO;
    let mut product_ids = Vec::new();
    let mut quantities = Vec::new();
    let mut prices = Vec::new();
    let mut product_names = Vec::new();
    let mut product_images = Vec::new();

    for item in &payload.items {
        if item.quantity <= 0 {
            return Err(AppError::BadRequest(format!(
                "Invalid quantity for product {}",
                item.product_id
            )));
        }

        let product = products_queries::find_by_id(&state.db, item.product_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Product {} not found", item.product_id)))?;

        if !product.enabled {
            return Err(AppError::BadRequest(format!(
                "Product {} is not available",
                item.product_id
            )));
        }

        if product.quantity < item.quantity {
            return Err(AppError::BadRequest(format!(
                "Insufficient stock for product {}",
                item.product_id
            )));
        }

        // Get primary image metadata for snapshot
        let images =
            products_queries::find_images_by_product_id(&state.db, item.product_id).await?;
        let image_json = images.first().map_or(serde_json::Value::Null, |img| {
            serde_json::json!({
                "product_id": img.product_id,
                "image_uuid": img.image_uuid,
                "color": img.color,
                "is_primary": img.is_primary,
                "extension": img.extension,
            })
        });

        let discounted_price = if product.discount > Decimal::ZERO {
            product.price * (Decimal::ONE - product.discount / Decimal::from(100))
        } else {
            product.price
        };

        let item_total = discounted_price * Decimal::from(item.quantity);
        total_amount += item_total;

        product_ids.push(item.product_id);
        quantities.push(item.quantity);
        prices.push(discounted_price);
        product_names.push(product.name.clone());
        product_images.push(image_json);
    }

    let amount_tetri = ((total_amount + DELIVERY_PRICE) * Decimal::from(100))
        .trunc()
        .to_string()
        .parse::<i32>()
        .map_err(|_| AppError::InternalError("Failed to calculate amount".to_string()))?;

    if amount_tetri <= 0 {
        return Err(AppError::BadRequest(
            "Order amount must be positive".to_string(),
        ));
    }

    let order_id = format!("tene_{}", Uuid::new_v4());

    let order =
        order_queries::create_order(&state.db, user_id, &order_id, amount_tetri, &payload).await?;

    order_queries::create_order_items(
        &state.db,
        order.id,
        &product_ids,
        &quantities,
        &prices,
        &product_names,
        &product_images,
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

    match order_queries::update_order_status(&state.db, order_id, order_status, payment_id).await {
        Ok(Some(order)) => {
            if order_status == "approved" {
                match order_queries::deduct_stock_for_order(&state.db, order.id).await {
                    Ok(true) => {}
                    Ok(false) => tracing::warn!("Insufficient stock for approved order {}", order_id),
                    Err(e) => tracing::error!("Failed to deduct stock for order {}: {:?}", order_id, e),
                }
            }
            StatusCode::OK
        }
        Ok(None) => {
            tracing::warn!("Flitt callback: order {} not found", order_id);
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
