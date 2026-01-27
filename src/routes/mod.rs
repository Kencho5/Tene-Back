mod google_auth;
mod health;
mod login;
mod products;
mod register;
mod send_code;
mod user_addresses;

use axum::{
    Router, middleware,
    routing::{get, post, put},
};

use crate::{AppState, middleware::auth_middleware};

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/health/ready", get(health::readiness_check))
        .nest("/auth", auth_routes())
        .merge(products_routes())
        .merge(user_routes())
        .merge(admin_routes())
}

fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register::register_user))
        .route("/login", post(login::login_user))
        .route("/google-login", post(google_auth::google_auth))
        .route("/send-code", post(send_code::send_verification_code))
        .route("/verify-code", post(send_code::verify_code))
}

fn products_routes() -> Router<AppState> {
    Router::new()
        .route("/products", get(products::search_product))
        .route("/products/facets", get(products::get_product_facets))
        .route("/products/{id}", get(products::get_product))
}

fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/addresses", get(user_addresses::get_address))
        .route("/addresses", post(user_addresses::add_address))
        .route("/addresses/{address_id}", put(user_addresses::edit_address))
        .layer(middleware::from_fn(auth_middleware))
}

fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/products", post(products::create_product))
        .route("/admin/products/{id}", put(products::update_product))
        .layer(middleware::from_fn(auth_middleware))
}
