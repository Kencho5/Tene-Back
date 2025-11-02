mod app_error;

pub use app_error::AppError;

pub type Result<T> = std::result::Result<T, AppError>;
