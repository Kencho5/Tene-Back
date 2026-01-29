use axum::{Json, extract::State};

use crate::{AppState, error::Result, models::CategoryTree, queries::category_queries};

pub async fn get_category_tree(State(state): State<AppState>) -> Result<Json<CategoryTree>> {
    let tree = category_queries::get_category_tree(&state.db, true).await?;
    Ok(Json(tree))
}

pub async fn get_all_categories(
    State(state): State<AppState>,
) -> Result<Json<Vec<crate::models::Category>>> {
    let categories = category_queries::get_all(&state.db, true).await?;
    Ok(Json(categories))
}
