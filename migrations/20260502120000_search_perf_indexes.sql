CREATE INDEX IF NOT EXISTS idx_product_categories_category_product
    ON product_categories(category_id, product_id);

CREATE INDEX IF NOT EXISTS idx_product_images_product_color
    ON product_images(product_id, color)
    WHERE color IS NOT NULL AND color <> '';

CREATE INDEX IF NOT EXISTS idx_products_enabled_created_at
    ON products(created_at DESC)
    WHERE enabled = true;

CREATE INDEX IF NOT EXISTS idx_products_enabled_quantity
    ON products(quantity)
    WHERE enabled = true AND quantity > 0;

CREATE INDEX IF NOT EXISTS idx_products_enabled_discount
    ON products(discount)
    WHERE enabled = true AND discount > 0;

CREATE INDEX IF NOT EXISTS idx_product_images_product_primary_created
    ON product_images(product_id, is_primary DESC, created_at ASC);
