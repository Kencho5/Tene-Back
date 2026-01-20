ALTER TABLE products ADD COLUMN brand VARCHAR(255);

CREATE INDEX idx_products_brand ON products(brand);
