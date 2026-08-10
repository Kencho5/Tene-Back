mod app_error;

pub use app_error::{AppError, SESSION_EXPIRED};

pub type Result<T> = std::result::Result<T, AppError>;
