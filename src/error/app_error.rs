use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    DatabaseError(sqlx::Error),
    ConfigError(String),
    InternalError(String),
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Unauthorized(String),
    Forbidden(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::DatabaseError(e) => write!(f, "მონაცემთა ბაზის შეცდომა: {}", e),
            AppError::ConfigError(msg) => write!(f, "კონფიგურაციის შეცდომა: {}", msg),
            AppError::InternalError(msg) => write!(f, "შიდა შეცდომა: {}", msg),
            AppError::NotFound(msg) => write!(f, "ვერ მოიძებნა: {}", msg),
            AppError::BadRequest(msg) => write!(f, "არასწორი მოთხოვნა: {}", msg),
            AppError::Conflict(msg) => write!(f, "კონფლიქტი: {}", msg),
            AppError::Unauthorized(msg) => write!(f, "არაავტორიზებული: {}", msg),
            AppError::Forbidden(msg) => write!(f, "აკრძალული: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::DatabaseError(err)
    }
}

impl From<std::env::VarError> for AppError {
    fn from(err: std::env::VarError) -> Self {
        AppError::ConfigError(err.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::DatabaseError(ref e) => {
                tracing::error!("Database error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "მონაცემთა ბაზის შეცდომა")
            }
            AppError::ConfigError(ref msg) => {
                tracing::error!("Configuration error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "სერვერის კონფიგურაციის შეცდომა",
                )
            }
            AppError::InternalError(ref msg) => {
                tracing::error!("Internal error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg.as_str())
            }
            AppError::NotFound(ref msg) => (StatusCode::NOT_FOUND, msg.as_str()),
            AppError::BadRequest(ref msg) => (StatusCode::BAD_REQUEST, msg.as_str()),
            AppError::Conflict(ref msg) => (StatusCode::CONFLICT, msg.as_str()),
            AppError::Unauthorized(ref msg) => (StatusCode::UNAUTHORIZED, msg.as_str()),
            AppError::Forbidden(ref msg) => (StatusCode::FORBIDDEN, msg.as_str()),
        };

        let body = Json(json!({
            "message": error_message,
        }));

        (status, body).into_response()
    }
}
