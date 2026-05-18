CREATE TABLE IF NOT EXISTS top_products (
    product_id TEXT PRIMARY KEY REFERENCES products(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CONSTRAINT top_products_position_unique UNIQUE (position) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT top_products_position_nonneg CHECK (position >= 0)
);

CREATE INDEX idx_top_products_position ON top_products(position);
