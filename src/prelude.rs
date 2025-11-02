// Standard library
pub use std::env;

// External crates
pub use http::{HeaderValue, Method};
//pub use serde::{Deserialize, Serialize};
//pub use serde_json::json;
pub use dotenv::dotenv;
pub use sqlx::postgres::{PgPool, PgPoolOptions};
pub use tower_http::cors::CorsLayer;
//pub use uuid::Uuid;

// Axum framework
pub use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    middleware,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};

// Local crate modules
pub use crate::{loaders::*, structs::app_state::*};
