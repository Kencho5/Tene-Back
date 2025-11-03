mod health;
mod product;

use axum::{Router, routing::get};

use crate::AppState;

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/health/ready", get(health::readiness_check))
        .route("/product/{id}", get(product::get_product))
}
