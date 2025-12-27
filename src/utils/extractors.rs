use crate::{
    error::{AppError, Result},
    utils::jwt::Claims,
};

pub fn extract_user_id(claims: &Claims) -> Result<i32> {
    claims.sub.parse::<i32>()
        .map_err(|_| AppError::Unauthorized("Unauthorized".to_string()))
}
