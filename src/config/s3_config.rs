use aws_config::{BehaviorVersion, Region};
use aws_sdk_s3::{Client as S3Client, config::Credentials};

use crate::error::{AppError, Result};

pub async fn load_s3_client() -> Result<S3Client> {
    let aws_access_key = std::env::var("AWS_ACCESS_KEY_ID")
        .map_err(|_| AppError::ConfigError("AWS_ACCESS_KEY_ID not set".to_string()))?;

    let aws_secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
        .map_err(|_| AppError::ConfigError("AWS_SECRET_ACCESS_KEY not set".to_string()))?;

    let aws_region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());

    let credentials = Credentials::new(
        aws_access_key,
        aws_secret_key,
        None,
        None,
        "env-credentials",
    );

    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(aws_region))
        .credentials_provider(credentials)
        .load()
        .await;

    let s3_client = S3Client::new(&config);

    tracing::info!("AWS S3 client initialized");

    Ok(s3_client)
}
