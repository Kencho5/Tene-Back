use axum::{extract::Request, middleware::Next, response::Response};

use crate::{
    error::{AppError, SESSION_EXPIRED},
    models::UserRole,
};

pub async fn auth_middleware(mut req: Request, next: Next) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or_else(|| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

    let claims = crate::utils::jwt::verify_token(token)
        .map_err(|_| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

pub async fn admin_middleware(mut req: Request, next: Next) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or_else(|| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

    let claims = crate::utils::jwt::verify_token(token)
        .map_err(|_| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

    if claims.role != UserRole::Admin {
        return Err(AppError::Forbidden(
            "ადმინისტრატორის წვდომა აუცილებელია".to_string(),
        ));
    }

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

pub async fn operator_middleware(mut req: Request, next: Next) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or_else(|| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

    let claims = crate::utils::jwt::verify_token(token)
        .map_err(|_| AppError::TokenInvalid(SESSION_EXPIRED.to_string()))?;

    if claims.role != UserRole::Admin && claims.role != UserRole::Operator {
        return Err(AppError::Forbidden(
            "ოპერატორის ან ადმინისტრატორის წვდომა აუცილებელია".to_string(),
        ));
    }

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
