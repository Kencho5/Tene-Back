ALTER TABLE products ADD COLUMN sku TEXT;

UPDATE products SET sku = id WHERE sku IS NULL;

ALTER TABLE products ALTER COLUMN sku SET NOT NULL;

CREATE UNIQUE INDEX idx_products_sku ON products(sku);
