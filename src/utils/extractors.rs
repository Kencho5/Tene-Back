use axum::{
    extract::FromRequestParts,
    http::request::Parts,
};

use crate::{
    AppState,
    error::{AppError, Result},
    utils::jwt::Claims,
};

pub fn extract_user_id(claims: &Claims) -> Result<i32> {
    claims.sub.parse::<i32>()
        .map_err(|_| AppError::Unauthorized("არაავტორიზებული".to_string()))
}

pub struct OptionalClaims(pub Option<Claims>);

impl FromRequestParts<AppState> for OptionalClaims {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &AppState) -> Result<Self> {
        let claims = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .and_then(|token| crate::utils::jwt::verify_token(token).ok());

        Ok(OptionalClaims(claims))
    }
}
