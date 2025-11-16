mod google_auth;
mod health;
mod login;
mod products;
mod register;
mod send_code;

use axum::{
    Router,
    routing::{get, post},
};

use crate::AppState;

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/product/{id}", get(products::get_product))
        .route("/health", get(health::health_check))
        .route("/health/ready", get(health::readiness_check))
        .nest("/auth", auth_routes())
}

fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register::register_user))
        .route("/login", post(login::login_user))
        .route("/google", post(google_auth::google_auth))
        .route("/send-code", post(send_code::send_verification_code))
        .route("/verify-code", post(send_code::verify_code))
}
