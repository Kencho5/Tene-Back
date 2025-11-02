use axum::{
    extract::DefaultBodyLimit,
    http::{HeaderValue, Method},
    Router,
};
use sqlx::PgPool;
use tower_http::cors::CorsLayer;

use crate::{config::AppConfig, database, error::Result, routes};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

pub async fn build(config: &AppConfig) -> Result<Router> {
    let pool = database::create_pool(&config.database).await?;
    let state = AppState { db: pool };
    let allowed_origins: Vec<HeaderValue> = config
        .cors
        .allowed_origins
        .iter()
        .map(|origin| {
            origin.parse::<HeaderValue>().map_err(|_| {
                crate::error::AppError::ConfigError(format!("Invalid CORS origin: {}", origin))
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            http::header::CONTENT_TYPE,
            http::header::AUTHORIZATION,
        ])
        .allow_origin(allowed_origins);

    let app = routes::create_router()
        .layer(DefaultBodyLimit::max(config.server.max_body_size))
        .layer(cors)
        .with_state(state);

    Ok(app)
}
