use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

use crate::{AppState, database, error::Result};

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({ "status": "ok" })))
}

pub async fn readiness_check(State(state): State<AppState>) -> Result<impl IntoResponse> {
    database::check_health(&state.db).await?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "status": "ready",
            "database": "connected"
        })),
    ))
}
