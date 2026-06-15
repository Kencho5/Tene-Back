mod admin;
mod blogs;
mod categories;
mod google_auth;
mod health;
mod login;
mod orders;
mod products;
mod register;
mod send_code;
mod tasks;
mod user_addresses;

use axum::{
    Router, middleware,
    routing::{delete, get, patch, post, put},
};

use crate::{
    AppState,
    middleware::{admin_middleware, auth_middleware, operator_middleware},
};

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health::health_check))
        .route("/health/ready", get(health::readiness_check))
        .nest("/auth", auth_routes())
        .merge(products_routes())
        .merge(categories_routes())
        .merge(blogs_routes())
        .merge(user_routes())
        .merge(checkout_routes())
        .route("/payments/callback", post(orders::flitt_callback))
        .route("/payments/redirect", get(orders::payment_redirect))
        .merge(admin_routes())
        .merge(operator_routes())
}

fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register::register_user))
        .route("/register/verify", post(register::verify_and_register))
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
        .route("/products/{id}/views", post(products::add_product_views))
        .route("/top-products", get(products::get_top_products))
        .route(
            "/products/{id}/related",
            get(products::get_related_products),
        )
        .route("/brands", get(products::get_brands))
        .route("/cable-types", get(products::get_cable_types))
        .route(
            "/cable-types/{id}",
            get(products::get_cable_type_with_variants),
        )
        .route(
            "/cable-types/{id}/variants",
            get(products::get_cable_variants),
        )
}

fn blogs_routes() -> Router<AppState> {
    Router::new()
        .route("/blogs", get(blogs::list_public_blogs))
        .route("/blogs/{slug}", get(blogs::get_public_blog))
}

fn categories_routes() -> Router<AppState> {
    Router::new()
        .route("/categories", get(categories::get_all_categories))
        .route("/categories/tree", get(categories::get_category_tree))
}

fn user_routes() -> Router<AppState> {
    Router::new()
        .route("/addresses", get(user_addresses::get_address))
        .route("/addresses", post(user_addresses::add_address))
        .route("/addresses/{address_id}", put(user_addresses::edit_address))
        .route(
            "/addresses/{address_id}",
            delete(user_addresses::delete_address),
        )
        .layer(middleware::from_fn(auth_middleware))
}

fn checkout_routes() -> Router<AppState> {
    let authed = Router::new()
        .route("/orders", get(orders::get_orders))
        .layer(middleware::from_fn(auth_middleware));

    Router::new()
        .route("/checkout", post(orders::checkout))
        .route(
            "/checkout/comment-images",
            put(orders::generate_comment_image_urls),
        )
        .route(
            "/checkout/analytics",
            post(orders::track_checkout_analytics),
        )
        .route("/orders/{id}", get(orders::get_order))
        .merge(authed)
}

fn admin_routes() -> Router<AppState> {
    Router::new()
        // products
        .route("/admin/products", get(admin::search_products))
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
        .route(
            "/admin/products/{id}/categories",
            put(admin::assign_categories_to_product),
        )
        // categories
        .route("/admin/categories", get(admin::get_all_categories_admin))
        .route(
            "/admin/categories/tree",
            get(admin::get_category_tree_admin),
        )
        .route("/admin/categories", post(admin::create_category))
        .route("/admin/categories/{id}", get(admin::get_category))
        .route("/admin/categories/{id}", put(admin::update_category))
        .route("/admin/categories/{id}", delete(admin::delete_category))
        .route(
            "/admin/categories/{id}/image",
            put(admin::generate_category_image_url),
        )
        .route(
            "/admin/categories/{id}/image/{image_uuid}",
            delete(admin::delete_category_image),
        )
        // brands
        .route("/admin/brands", get(admin::get_brands))
        .route("/admin/brands", post(admin::create_brand))
        .route("/admin/brands/{id}", put(admin::update_brand))
        .route("/admin/brands/{id}", delete(admin::delete_brand))
        // cable types
        .route("/admin/cable-types", get(admin::get_cable_types))
        .route("/admin/cable-types", post(admin::create_cable_type))
        .route("/admin/cable-types/{id}", put(admin::update_cable_type))
        .route("/admin/cable-types/{id}", delete(admin::delete_cable_type))
        // cable variants
        .route(
            "/admin/cable-types/{type_id}/variants",
            get(admin::get_cable_variants),
        )
        .route(
            "/admin/cable-types/{type_id}/variants",
            post(admin::create_cable_variant),
        )
        .route(
            "/admin/cable-types/{type_id}/variants/{variant_id}",
            put(admin::update_cable_variant),
        )
        .route(
            "/admin/cable-types/{type_id}/variants/{variant_id}",
            delete(admin::delete_cable_variant),
        )
        // tasks
        .route("/admin/tasks", get(tasks::search_tasks))
        .route("/admin/tasks", post(tasks::create_task))
        .route("/admin/tasks/{id}", get(tasks::get_task))
        .route("/admin/tasks/{id}", put(tasks::update_task))
        .route("/admin/tasks/{id}", delete(tasks::delete_task))
        .route("/admin/tasks/{id}/state", patch(tasks::update_task_state))
        .route(
            "/admin/tasks/{id}/media",
            put(tasks::generate_task_media_urls),
        )
        .route(
            "/admin/tasks/{id}/media/{media_uuid}",
            delete(tasks::delete_task_media),
        )
        // blogs
        .route("/admin/blogs", get(blogs::search_blogs))
        .route("/admin/blogs", post(blogs::create_blog))
        .route("/admin/blogs/{id}", get(blogs::get_blog))
        .route("/admin/blogs/{id}", put(blogs::update_blog))
        .route("/admin/blogs/{id}", delete(blogs::delete_blog))
        .route(
            "/admin/blogs/{id}/media",
            put(blogs::generate_blog_media_urls),
        )
        .route(
            "/admin/blogs/{id}/media/{media_uuid}",
            delete(blogs::delete_blog_media),
        )
        .route(
            "/admin/blogs/{id}/media/{media_uuid}/thumbnail",
            patch(blogs::set_blog_media_thumbnail),
        )
        // analytics
        .route("/admin/analytics", get(admin::get_analytics))
        .route(
            "/admin/checkout-sessions",
            get(admin::get_checkout_sessions),
        )
        // top products
        .route("/admin/top-products", get(admin::get_top_products_admin))
        .route("/admin/top-products", put(admin::replace_top_products))
        // users
        .route("/admin/users", get(admin::search_users))
        .route("/admin/users/{id}", put(admin::update_user))
        .route("/admin/users/{id}", delete(admin::delete_user))
        .layer(middleware::from_fn(admin_middleware))
}

fn operator_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/orders", get(admin::get_orders))
        .route("/admin/orders/export", get(admin::export_orders))
        .route(
            "/admin/orders/{id}/status",
            patch(admin::update_order_status),
        )
        .route(
            "/admin/orders/payment-link",
            post(admin::create_payment_link),
        )
        .layer(middleware::from_fn(operator_middleware))
}
