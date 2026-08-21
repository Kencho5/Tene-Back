use axum::{extract::FromRequestParts, http::request::Parts};

use crate::{
    AppState,
    error::{AppError, Result, SESSION_EXPIRED},
    utils::jwt::Claims,
};

pub fn extract_user_id(claims: &Claims) -> Result<i32> {
    claims
        .sub
        .parse::<i32>()
        .map_err(|_| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))
}

pub struct OptionalClaims(pub Option<Claims>);

impl FromRequestParts<AppState> for OptionalClaims {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &AppState) -> Result<Self> {
        let Some(header) = parts.headers.get(axum::http::header::AUTHORIZATION) else {
            return Ok(OptionalClaims(None));
        };

        let token = header
            .to_str()
            .ok()
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

        if token.trim().is_empty() {
            return Ok(OptionalClaims(None));
        }

        let claims = crate::utils::jwt::verify_token(token)
            .map_err(|_| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

        Ok(OptionalClaims(Some(claims)))
    }
}

pub struct LenientClaims(pub Option<Claims>);

impl FromRequestParts<AppState> for LenientClaims {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &AppState) -> Result<Self> {
        let claims = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .and_then(|token| crate::utils::jwt::verify_token(token).ok());

        Ok(LenientClaims(claims))
    }
}
