use crate::error::{AppError, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct FlittConfig {
    pub merchant_id: i32,
    pub secret_key: String,
    pub backend_url: String,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub cors: CorsConfig,
    pub s3: S3Config,
    pub environment: Environment,
    pub flitt: FlittConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Environment {
    Staging,
    Main,
}

impl Environment {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "staging" => Ok(Environment::Staging),
            "main" => Ok(Environment::Main),
            _ => Err(AppError::ConfigError(format!(
                "Invalid environment: {}. Must be 'staging' or 'main'",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct S3Config {
    pub bucket: String,
    pub assets_url: String,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub max_body_size: usize,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
}

#[derive(Debug, Clone)]
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let environment = Environment::from_str(
            &env::var("ENVIRONMENT").unwrap_or_else(|_| "staging".to_string()),
        )?;

        Ok(Self {
            server: ServerConfig {
                host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: env::var("PORT")
                    .unwrap_or_else(|_| "3000".to_string())
                    .parse()
                    .map_err(|_| AppError::ConfigError("Invalid PORT value".to_string()))?,
                max_body_size: env::var("MAX_BODY_SIZE")
                    .unwrap_or_else(|_| "10485760".to_string())
                    .parse()
                    .map_err(|_| {
                        AppError::ConfigError("Invalid MAX_BODY_SIZE value".to_string())
                    })?,
            },
            database: DatabaseConfig {
                url: env::var("DB_URL")?,
                max_connections: env::var("DB_MAX_CONNECTIONS")
                    .unwrap_or_else(|_| "20".to_string())
                    .parse()
                    .map_err(|_| {
                        AppError::ConfigError("Invalid DB_MAX_CONNECTIONS value".to_string())
                    })?,
            },
            cors: CorsConfig {
                allowed_origins: env::var("FRONTEND_URL")?
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect(),
            },
            s3: S3Config {
                bucket: env::var("S3_BUCKET")
                    .map_err(|_| AppError::ConfigError("S3_BUCKET not set".to_string()))?,
                assets_url: env::var("ASSETS_URL")
                    .map_err(|_| AppError::ConfigError("ASSETS_URL not set".to_string()))?,
            },
            flitt: FlittConfig {
                merchant_id: env::var("FLITT_MERCHANT_ID")
                    .map_err(|_| AppError::ConfigError("FLITT_MERCHANT_ID not set".to_string()))?
                    .parse()
                    .map_err(|_| {
                        AppError::ConfigError("Invalid FLITT_MERCHANT_ID value".to_string())
                    })?,
                secret_key: env::var("FLITT_SECRET_KEY").map_err(|_| {
                    AppError::ConfigError("FLITT_SECRET_KEY not set".to_string())
                })?,
                backend_url: env::var("BACKEND_URL").map_err(|_| {
                    AppError::ConfigError("BACKEND_URL not set".to_string())
                })?,
            },
            environment,
        })
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}
