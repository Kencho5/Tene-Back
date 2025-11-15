use aws_config::{BehaviorVersion, Region};
use aws_sdk_sesv2::{config::Credentials, Client as SesClient};

use crate::error::{AppError, Result};

pub async fn load_ses_client() -> Result<SesClient> {
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

    let ses_client = SesClient::new(&config);

    tracing::info!("AWS SES client initialized");

    Ok(ses_client)
}
