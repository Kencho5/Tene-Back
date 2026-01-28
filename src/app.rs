pub use aws_sdk_s3 as s3;
pub use aws_sdk_sesv2::Client as SesClient;
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderValue, Method},
};
use sqlx::PgPool;
use tower_http::cors::CorsLayer;

use crate::{config, config::AppConfig, database, error::Result, routes};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub s3_client: s3::Client,
    pub s3_bucket: String,
    pub assets_url: String,
    pub environment: config::Environment,
    pub ses_client: SesClient,
}

pub async fn build(config: &AppConfig) -> Result<Router> {
    let pool = database::create_pool(&config.database).await?;
    let ses_client = config::load_ses_client().await?;
    let s3_client = config::load_s3_client().await?;

    let state = AppState {
        db: pool,
        s3_client,
        s3_bucket: config.s3.bucket.clone(),
        assets_url: config.s3.assets_url.clone(),
        environment: config.environment.clone(),
        ses_client,
    };
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
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::OPTIONS,
            Method::DELETE,
        ])
        .allow_headers([http::header::CONTENT_TYPE, http::header::AUTHORIZATION])
        .allow_origin(allowed_origins);

    let app = routes::create_router()
        .layer(DefaultBodyLimit::max(config.server.max_body_size))
        .layer(cors)
        .with_state(state);

    Ok(app)
}
