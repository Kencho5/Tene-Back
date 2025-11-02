use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::{database, error::Result, AppState};

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
