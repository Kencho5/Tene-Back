use axum::{extract::State, Json};

use crate::{
    error::Result,
    models::{CategoryResponse, CategoryResponseWithChildren, CategoryTreeResponse},
    queries::category_queries,
    AppState,
};

pub async fn get_category_tree(
    State(state): State<AppState>,
) -> Result<Json<CategoryTreeResponse>> {
    let tree = category_queries::get_category_tree(&state.db, true).await?;

    // Collect all category IDs from the tree
    fn collect_ids(
        nodes: &[crate::models::CategoryWithChildren],
        ids: &mut Vec<i32>,
    ) {
        for node in nodes {
            ids.push(node.category.id);
            collect_ids(&node.children, ids);
        }
    }

    let mut category_ids = Vec::new();
    collect_ids(&tree.categories, &mut category_ids);

    // Fetch all images
    let images = category_queries::get_category_images(&state.db, &category_ids).await?;

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "categories-staging",
        crate::config::Environment::Main => "categories-main",
    };

    // Build image URL helper
    let build_image_url = |category_id: i32| -> Option<String> {
        images.get(&category_id).map(|img| {
            format!(
                "{}/{}/{}/{}.{}",
                state.assets_url, env_prefix, category_id, img.image_uuid, img.extension
            )
        })
    };

    // Convert tree to response with image URLs
    fn build_response_tree(
        nodes: Vec<crate::models::CategoryWithChildren>,
        build_url: &dyn Fn(i32) -> Option<String>,
    ) -> Vec<CategoryResponseWithChildren> {
        nodes
            .into_iter()
            .map(|node| CategoryResponseWithChildren {
                image_url: build_url(node.category.id),
                children: build_response_tree(node.children, build_url),
                category: node.category,
            })
            .collect()
    }

    let response_categories = build_response_tree(tree.categories, &build_image_url);

    Ok(Json(CategoryTreeResponse {
        categories: response_categories,
    }))
}

pub async fn get_all_categories(
    State(state): State<AppState>,
) -> Result<Json<Vec<CategoryResponse>>> {
    let categories = category_queries::get_all(&state.db, true).await?;

    let category_ids: Vec<i32> = categories.iter().map(|c| c.id).collect();
    let images = category_queries::get_category_images(&state.db, &category_ids).await?;

    let env_prefix = match state.environment {
        crate::config::Environment::Staging => "categories-staging",
        crate::config::Environment::Main => "categories-main",
    };

    let response: Vec<CategoryResponse> = categories
        .into_iter()
        .map(|category| {
            let image_url = images.get(&category.id).map(|img| {
                format!(
                    "{}/{}/{}/{}.{}",
                    state.assets_url, env_prefix, category.id, img.image_uuid, img.extension
                )
            });

            CategoryResponse {
                category,
                image_url,
            }
        })
        .collect();

    Ok(Json(response))
}
