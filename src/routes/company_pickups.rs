use axum::{
    Extension, Json,
    extract::{Path, Query, State},
};
use http::StatusCode;
use rust_decimal::Decimal;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{
        Company, CompanyInvoice, CompanyInvoiceDetail, CompanyPickupRequest, InvoiceDelivery,
        InvoicePaymentRequest, InvoiceQuery, InvoiceSearchResponse, PickupConfirmResponse,
        PickupQuery, PickupSearchResponse, PickupStatus,
    },
    queries::{
        company_pickup_queries::{self, NewPickup},
        company_queries,
    },
    services::company_invoice_service::{self, PickupPricing},
    utils::jwt::Claims,
};

fn pickup_not_found() -> AppError {
    AppError::NotFound("გატანა ვერ მოიძებნა".to_string())
}

fn invoice_not_found() -> AppError {
    AppError::NotFound("ინვოისი ვერ მოიძებნა".to_string())
}

struct PreparedPickup {
    company: Company,
    address_snapshot: Option<String>,
    pricing: PickupPricing,
}

async fn prepare_pickup(state: &AppState, req: &CompanyPickupRequest) -> Result<PreparedPickup> {
    if req.quantity_kg <= Decimal::ZERO {
        return Err(AppError::BadRequest(
            "რაოდენობა 0-ზე მეტი უნდა იყოს".to_string(),
        ));
    }
    if req.pickup_date > company_invoice_service::today() {
        return Err(AppError::BadRequest(
            "გატანის თარიღი მომავალში ვერ იქნება".to_string(),
        ));
    }

    let company = company_queries::find_company(&state.db, req.company_id)
        .await?
        .ok_or_else(|| AppError::BadRequest("კომპანია ვერ მოიძებნა".to_string()))?;

    let address_snapshot = match req.address_id {
        Some(address_id) => Some(
            company_queries::find_address(&state.db, company.id, address_id)
                .await?
                .ok_or_else(|| {
                    AppError::BadRequest("მისამართი ამ კომპანიას არ ეკუთვნის".to_string())
                })?
                .display(),
        ),
        None => None,
    };

    let pricing = company_invoice_service::price_pickup(
        &state.db,
        company.id,
        req.waste_type,
        req.pickup_date,
        req.quantity_kg,
        req.amount,
    )
    .await?;

    Ok(PreparedPickup {
        company,
        address_snapshot,
        pricing,
    })
}

fn new_pickup<'a>(req: &'a CompanyPickupRequest, prepared: &'a PreparedPickup) -> NewPickup<'a> {
    NewPickup {
        company_id: prepared.company.id,
        address_id: req.address_id,
        address_snapshot: prepared.address_snapshot.as_deref(),
        pickup_date: req.pickup_date,
        waste_type: req.waste_type,
        quantity_kg: req.quantity_kg.round_dp(2),
        tariff_id: prepared.pricing.tariff_id,
        unit_price: prepared.pricing.unit_price,
        fixed_fee: prepared.pricing.fixed_fee,
        amount: prepared.pricing.amount,
        amount_overridden: prepared.pricing.amount_overridden,
        notes: req.notes.as_deref().filter(|n| !n.trim().is_empty()),
    }
}

pub async fn search_pickups(
    State(state): State<AppState>,
    Query(params): Query<PickupQuery>,
) -> Result<Json<PickupSearchResponse>> {
    let (pickups, total, total_quantity_kg, total_amount, limit, offset) =
        company_pickup_queries::search_pickups(&state.db, &params, true).await?;
    Ok(Json(PickupSearchResponse {
        pickups,
        total,
        total_quantity_kg,
        total_amount,
        limit,
        offset,
    }))
}

pub async fn create_pickup(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<CompanyPickupRequest>,
) -> Result<(StatusCode, Json<PickupConfirmResponse>)> {
    let prepared = prepare_pickup(&state, &payload).await?;
    let id = company_pickup_queries::insert_pickup(
        &state.db,
        &new_pickup(&payload, &prepared),
        claims.user_id,
    )
    .await?;

    if !prepared.company.requires_confirmation {
        let confirmed = company_invoice_service::confirm_pickup(&state, id, claims.user_id).await?;
        return Ok((StatusCode::CREATED, Json(confirmed)));
    }

    let pickup = company_pickup_queries::find_pickup(&state.db, id)
        .await?
        .ok_or_else(pickup_not_found)?;
    Ok((
        StatusCode::CREATED,
        Json(PickupConfirmResponse {
            pickup,
            invoice: None,
            delivery: None,
        }),
    ))
}

async fn ensure_pending(state: &AppState, id: i32) -> Result<()> {
    let pickup = company_pickup_queries::find_pickup(&state.db, id)
        .await?
        .ok_or_else(pickup_not_found)?;
    if pickup.status == PickupStatus::Confirmed {
        return Err(AppError::Conflict(
            "დადასტურებული გატანის შეცვლა შეუძლებელია".to_string(),
        ));
    }
    Ok(())
}

pub async fn update_pickup(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<CompanyPickupRequest>,
) -> Result<Json<PickupConfirmResponse>> {
    ensure_pending(&state, id).await?;
    let prepared = prepare_pickup(&state, &payload).await?;
    if !company_pickup_queries::update_pending_pickup(
        &state.db,
        id,
        &new_pickup(&payload, &prepared),
    )
    .await?
    {
        ensure_pending(&state, id).await?;
    }
    let pickup = company_pickup_queries::find_pickup(&state.db, id)
        .await?
        .ok_or_else(pickup_not_found)?;
    Ok(Json(PickupConfirmResponse {
        pickup,
        invoice: None,
        delivery: None,
    }))
}

pub async fn delete_pickup(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<StatusCode> {
    if company_pickup_queries::delete_pending_pickup(&state.db, id).await? == 0 {
        ensure_pending(&state, id).await?;
        return Err(pickup_not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn confirm_pickup(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i32>,
) -> Result<Json<PickupConfirmResponse>> {
    let confirmed = company_invoice_service::confirm_pickup(&state, id, claims.user_id).await?;
    Ok(Json(confirmed))
}

pub async fn search_invoices(
    State(state): State<AppState>,
    Query(params): Query<InvoiceQuery>,
) -> Result<Json<InvoiceSearchResponse>> {
    let (invoices, total, limit, offset) =
        company_pickup_queries::search_invoices(&state.db, &params, true).await?;
    Ok(Json(InvoiceSearchResponse {
        invoices,
        total,
        limit,
        offset,
    }))
}

pub async fn get_invoice(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<CompanyInvoiceDetail>> {
    let invoice = company_pickup_queries::find_invoice(&state.db, id)
        .await?
        .ok_or_else(invoice_not_found)?;
    let html = company_pickup_queries::invoice_html(&state.db, id)
        .await?
        .unwrap_or_default();
    let deliveries = company_pickup_queries::list_deliveries(&state.db, id).await?;
    Ok(Json(CompanyInvoiceDetail {
        invoice,
        html,
        deliveries,
    }))
}

pub async fn send_invoice(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i32>,
) -> Result<Json<InvoiceDelivery>> {
    let delivery =
        company_invoice_service::deliver_invoice(&state, id, Some(claims.user_id)).await?;
    Ok(Json(delivery))
}

pub async fn set_invoice_payment(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(payload): Json<InvoicePaymentRequest>,
) -> Result<Json<CompanyInvoice>> {
    if !company_pickup_queries::set_invoice_paid(&state.db, id, payload.paid).await? {
        return Err(invoice_not_found());
    }
    company_pickup_queries::find_invoice(&state.db, id)
        .await?
        .map(Json)
        .ok_or_else(invoice_not_found)
}
