use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use rand::Rng;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{
        PhoneNumberRequest, PhoneVerificationTokenResponse, UserPhoneNumber, VerifyPhoneRequest,
    },
    queries::phone_queries,
    services::sms_service,
    utils::{
        extractors::extract_user_id,
        jwt::{Claims, generate_phone_verification_token},
        phone::normalize_phone,
    },
};

pub async fn send_code(
    State(state): State<AppState>,
    Json(payload): Json<PhoneNumberRequest>,
) -> Result<StatusCode> {
    let phone_number = normalize_phone(&payload.phone_number)?;
    let code = rand::rng().random_range(100000..1000000);

    phone_queries::create_verification_code(&state.db, &phone_number, code).await?;

    sms_service::send_verification_code(&state.sms_api_key, &state.sms_sender, &phone_number, code)
        .await?;

    Ok(StatusCode::OK)
}

pub async fn verify_code(
    State(state): State<AppState>,
    Json(payload): Json<VerifyPhoneRequest>,
) -> Result<Json<PhoneVerificationTokenResponse>> {
    let phone_number = normalize_phone(&payload.phone_number)?;

    phone_queries::check_verification_code(&state.db, &phone_number, payload.code).await?;
    phone_queries::consume_verification_codes(&state.db, &phone_number).await?;

    let verification_token =
        generate_phone_verification_token(&phone_number, chrono::Duration::minutes(30))?;

    Ok(Json(PhoneVerificationTokenResponse { verification_token }))
}

pub async fn verify_phone(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<VerifyPhoneRequest>,
) -> Result<Json<UserPhoneNumber>> {
    let user_id = extract_user_id(&claims)?;
    let phone_number = normalize_phone(&payload.phone_number)?;

    phone_queries::check_verification_code(&state.db, &phone_number, payload.code).await?;
    phone_queries::consume_verification_codes(&state.db, &phone_number).await?;

    let phone = phone_queries::mark_phone_verified(&state.db, user_id, &phone_number).await?;

    Ok(Json(phone))
}

pub async fn get_phone_numbers(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<UserPhoneNumber>>> {
    let user_id = extract_user_id(&claims)?;

    let phones = phone_queries::get_user_phone_numbers(&state.db, user_id).await?;

    Ok(Json(phones))
}

pub async fn add_phone_number(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<PhoneNumberRequest>,
) -> Result<Json<UserPhoneNumber>> {
    let user_id = extract_user_id(&claims)?;
    let phone_number = normalize_phone(&payload.phone_number)?;

    let phone = phone_queries::add_user_phone_number(&state.db, user_id, &phone_number).await?;

    Ok(Json(phone))
}

pub async fn update_phone_number(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i32>,
    Json(payload): Json<PhoneNumberRequest>,
) -> Result<Json<UserPhoneNumber>> {
    let user_id = extract_user_id(&claims)?;
    let phone_number = normalize_phone(&payload.phone_number)?;

    let phone = phone_queries::update_user_phone_number(&state.db, user_id, id, &phone_number)
        .await?
        .ok_or(AppError::NotFound("ნომერი ვერ მოიძებნა".to_string()))?;

    Ok(Json(phone))
}

pub async fn delete_phone_number(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<i32>,
) -> Result<Json<UserPhoneNumber>> {
    let user_id = extract_user_id(&claims)?;

    let phone = phone_queries::delete_user_phone_number(&state.db, user_id, id)
        .await?
        .ok_or(AppError::NotFound("ნომერი ვერ მოიძებნა".to_string()))?;

    Ok(Json(phone))
}
