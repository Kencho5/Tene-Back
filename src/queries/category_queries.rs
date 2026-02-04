use std::collections::HashMap;

use sqlx::PgPool;

use uuid::Uuid;

use crate::{
    error::Result,
    models::{
        Category, CategoryFacetValue, CategoryImage, CategoryTree, CategoryWithChildren,
        CreateCategoryRequest, UpdateCategoryRequest,
    },
};

/// Find category by ID
pub async fn find_by_id(pool: &PgPool, id: i32) -> Result<Option<Category>> {
    let category = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;

    Ok(category)
}

/// Find category by slug
pub async fn find_by_slug(pool: &PgPool, slug: &str) -> Result<Option<Category>> {
    let category = sqlx::query_as::<_, Category>("SELECT * FROM categories WHERE slug = $1")
        .bind(slug)
        .fetch_optional(pool)
        .await?;

    Ok(category)
}

/// Get all categories (flat list)
pub async fn get_all(pool: &PgPool, enabled_only: bool) -> Result<Vec<Category>> {
    let query = if enabled_only {
        "SELECT * FROM categories WHERE enabled = true ORDER BY display_order ASC, name ASC"
    } else {
        "SELECT * FROM categories ORDER BY display_order ASC, name ASC"
    };

    let categories = sqlx::query_as::<_, Category>(query).fetch_all(pool).await?;

    Ok(categories)
}

/// Get category tree (hierarchical structure)
pub async fn get_category_tree(pool: &PgPool, enabled_only: bool) -> Result<CategoryTree> {
    let categories = get_all(pool, enabled_only).await?;

    // Group categories by parent_id
    let mut children_map: HashMap<Option<i32>, Vec<Category>> = HashMap::new();
    for category in categories {
        children_map
            .entry(category.parent_id)
            .or_default()
            .push(category);
    }

    // Build tree recursively
    fn build_tree(
        parent_id: Option<i32>,
        children_map: &HashMap<Option<i32>, Vec<Category>>,
    ) -> Vec<CategoryWithChildren> {
        children_map
            .get(&parent_id)
            .map(|categories| {
                categories
                    .iter()
                    .map(|cat| CategoryWithChildren {
                        children: build_tree(Some(cat.id), children_map),
                        category: cat.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    let root_categories = build_tree(None, &children_map);

    Ok(CategoryTree {
        categories: root_categories,
    })
}

/// Get categories for a specific product
pub async fn get_product_categories(pool: &PgPool, product_id: i32) -> Result<Vec<Category>> {
    let categories = sqlx::query_as::<_, Category>(
        "SELECT c.* FROM categories c
         INNER JOIN product_categories pc ON c.id = pc.category_id
         WHERE pc.product_id = $1
         ORDER BY c.display_order ASC, c.name ASC",
    )
    .bind(product_id)
    .fetch_all(pool)
    .await?;

    Ok(categories)
}

/// Create a new category
pub async fn create_category(
    pool: &PgPool,
    req: CreateCategoryRequest,
) -> Result<Category> {
    let category = sqlx::query_as::<_, Category>(
        "INSERT INTO categories (parent_id, name, slug, description, display_order, enabled)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING *",
    )
    .bind(req.parent_id)
    .bind(&req.name)
    .bind(&req.slug)
    .bind(&req.description)
    .bind(req.display_order.unwrap_or(0))
    .bind(req.enabled.unwrap_or(true))
    .fetch_one(pool)
    .await?;

    Ok(category)
}

/// Update an existing category
pub async fn update_category(
    pool: &PgPool,
    id: i32,
    req: UpdateCategoryRequest,
) -> Result<Option<Category>> {
    let mut query_builder = sqlx::QueryBuilder::<sqlx::Postgres>::new("UPDATE categories SET ");
    let mut has_fields = false;

    if let Some(parent_id) = req.parent_id {
        if has_fields {
            query_builder.push(", ");
        }
        query_builder.push("parent_id = ");
        query_builder.push_bind(parent_id);
        has_fields = true;
    }

    if let Some(name) = req.name {
        if has_fields {
            query_builder.push(", ");
        }
        query_builder.push("name = ");
        query_builder.push_bind(name);
        has_fields = true;
    }

    if let Some(slug) = req.slug {
        if has_fields {
            query_builder.push(", ");
        }
        query_builder.push("slug = ");
        query_builder.push_bind(slug);
        has_fields = true;
    }

    if let Some(description) = req.description {
        if has_fields {
            query_builder.push(", ");
        }
        query_builder.push("description = ");
        query_builder.push_bind(description);
        has_fields = true;
    }

    if let Some(display_order) = req.display_order {
        if has_fields {
            query_builder.push(", ");
        }
        query_builder.push("display_order = ");
        query_builder.push_bind(display_order);
        has_fields = true;
    }

    if let Some(enabled) = req.enabled {
        if has_fields {
            query_builder.push(", ");
        }
        query_builder.push("enabled = ");
        query_builder.push_bind(enabled);
        has_fields = true;
    }

    if !has_fields {
        // No fields to update, return existing category
        return find_by_id(pool, id).await;
    }

    query_builder.push(", updated_at = NOW() WHERE id = ");
    query_builder.push_bind(id);
    query_builder.push(" RETURNING *");

    let category = query_builder
        .build_query_as::<Category>()
        .fetch_optional(pool)
        .await?;

    Ok(category)
}

/// Delete a category
pub async fn delete_category(pool: &PgPool, id: i32) -> Result<bool> {
    let result = sqlx::query("DELETE FROM categories WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Assign categories to a product
pub async fn assign_categories_to_product(
    pool: &PgPool,
    product_id: i32,
    category_ids: &[i32],
) -> Result<()> {
    // First, remove existing associations
    sqlx::query("DELETE FROM product_categories WHERE product_id = $1")
        .bind(product_id)
        .execute(pool)
        .await?;

    // Then insert new associations
    if !category_ids.is_empty() {
        let mut query_builder =
            sqlx::QueryBuilder::new("INSERT INTO product_categories (product_id, category_id) ");

        query_builder.push_values(category_ids, |mut b, category_id| {
            b.push_bind(product_id).push_bind(category_id);
        });

        query_builder.build().execute(pool).await?;
    }

    Ok(())
}

/// Get category facets for product search
pub async fn get_category_facets(
    pool: &PgPool,
    enabled_products_only: bool,
) -> Result<Vec<CategoryFacetValue>> {
    let query = if enabled_products_only {
        "SELECT
            c.id,
            c.name,
            COUNT(DISTINCT p.id)::bigint as count
         FROM categories c
         INNER JOIN product_categories pc ON c.id = pc.category_id
         INNER JOIN products p ON pc.product_id = p.id
         WHERE c.enabled = true AND p.enabled = true
         GROUP BY c.id, c.name
         HAVING COUNT(DISTINCT p.id) > 0
         ORDER BY c.display_order ASC, c.name ASC
         LIMIT 100"
    } else {
        "SELECT
            c.id,
            c.name,
            COUNT(DISTINCT p.id)::bigint as count
         FROM categories c
         INNER JOIN product_categories pc ON c.id = pc.category_id
         INNER JOIN products p ON pc.product_id = p.id
         WHERE c.enabled = true
         GROUP BY c.id, c.name
         HAVING COUNT(DISTINCT p.id) > 0
         ORDER BY c.display_order ASC, c.name ASC
         LIMIT 100"
    };

    let facets = sqlx::query_as::<_, CategoryFacetValue>(query)
        .fetch_all(pool)
        .await?;

    Ok(facets)
}

/// Get category image
pub async fn get_category_image(
    pool: &PgPool,
    category_id: i32,
) -> Result<Option<CategoryImage>> {
    let image = sqlx::query_as::<_, CategoryImage>(
        "SELECT * FROM category_images WHERE category_id = $1 LIMIT 1",
    )
    .bind(category_id)
    .fetch_optional(pool)
    .await?;

    Ok(image)
}

/// Get category images for multiple categories
pub async fn get_category_images(
    pool: &PgPool,
    category_ids: &[i32],
) -> Result<HashMap<i32, CategoryImage>> {
    if category_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let images = sqlx::query_as::<_, CategoryImage>(
        "SELECT * FROM category_images WHERE category_id = ANY($1)",
    )
    .bind(category_ids)
    .fetch_all(pool)
    .await?;

    let image_map = images
        .into_iter()
        .map(|img| (img.category_id, img))
        .collect();

    Ok(image_map)
}

/// Add category image
pub async fn add_category_image(
    pool: &PgPool,
    category_id: i32,
    image_uuid: Uuid,
    extension: &str,
) -> Result<CategoryImage> {
    // Delete any existing image for this category first
    sqlx::query("DELETE FROM category_images WHERE category_id = $1")
        .bind(category_id)
        .execute(pool)
        .await?;

    let image = sqlx::query_as::<_, CategoryImage>(
        "INSERT INTO category_images (category_id, image_uuid, extension)
         VALUES ($1, $2, $3)
         RETURNING *",
    )
    .bind(category_id)
    .bind(image_uuid)
    .bind(extension)
    .fetch_one(pool)
    .await?;

    Ok(image)
}

/// Delete category image
pub async fn delete_category_image(
    pool: &PgPool,
    category_id: i32,
    image_uuid: Uuid,
) -> Result<bool> {
    let result = sqlx::query(
        "DELETE FROM category_images WHERE category_id = $1 AND image_uuid = $2",
    )
    .bind(category_id)
    .bind(image_uuid)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}
