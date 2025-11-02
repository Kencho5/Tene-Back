use crate::error::{AppError, Result};
use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub cors: CorsConfig,
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
                    .map_err(|_| AppError::ConfigError("Invalid MAX_BODY_SIZE value".to_string()))?,
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
        })
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }
}
