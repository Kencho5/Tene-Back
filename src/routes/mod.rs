mod admin;
mod google_auth;
mod health;
mod login;
mod products;
mod register;
mod send_code;
mod user_addresses;

use axum::{
    Router, middleware,
    routing::{delete, get, patch, post, put},
};

use crate::{
    AppState,
    middleware::{admin_middleware, auth_middleware},
};

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
        //ADMIN PRODUCTS
        .route("/admin/products", post(admin::create_product))
        .route("/admin/products/{id}", put(admin::update_product))
        .route("/admin/products/{id}", delete(admin::delete_product))
        .route(
            "/admin/products/{id}/images",
            put(admin::generate_product_urls),
        )
        .route(
            "/admin/products/{id}/images/{image_uuid}",
            delete(admin::delete_product_image),
        )
        .route(
            "/admin/products/{id}/images/{image_uuid}",
            patch(admin::update_product_image_metadata),
        )
        //ADMIN USERS
        .route("/admin/users", get(admin::search_users))
        .layer(middleware::from_fn(admin_middleware))
}
