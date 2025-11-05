mod google_auth;
mod health;
mod product;
mod register;

use axum::{
    routing::{get, post},
    Router,
};

use crate::AppState;

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/product/{id}", get(product::get_product))
        .route("/health", get(health::health_check))
        .route("/health/ready", get(health::readiness_check))
        .nest("/auth", auth_routes())
}

fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register::register_user))
        .route("/google", post(google_auth::google_auth))
}
