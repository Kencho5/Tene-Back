#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

mod loaders;
mod prelude;
mod register_routes;
mod structs;

use crate::prelude::*;
use tracing::Level;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let frontend_url = env::var("FRONTEND_URL").expect("Frontend url not set");
    let pool = init_db::init_db().await;

    let state = AppState { pool: pool };

    let app = register_routes::create_router()
        .layer(DefaultBodyLimit::max(10 * 1_000_000))
        .layer(
            CorsLayer::new()
                .allow_origin(frontend_url.parse::<HeaderValue>().unwrap())
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION]),
        )
        .with_state(state);

    println!("Server starting on 0.0.0.0:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
