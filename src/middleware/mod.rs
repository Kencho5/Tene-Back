use axum::{extract::Request, middleware::Next, response::Response};

use crate::{error::AppError, models::UserRole};

pub async fn auth_middleware(mut req: Request, next: Next) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Unauthorized("Invalid token format".to_string()))?;

    let claims = crate::utils::jwt::verify_token(token)?;

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

pub async fn admin_middleware(mut req: Request, next: Next) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Unauthorized("Invalid token format".to_string()))?;

    let claims = crate::utils::jwt::verify_token(token)?;

    if claims.role != UserRole::Admin {
        return Err(AppError::Forbidden(
            "Admin access required".to_string(),
        ));
    }

    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
