CREATE INDEX IF NOT EXISTS idx_product_views_viewed_at_product
    ON product_views(viewed_at, product_id);
