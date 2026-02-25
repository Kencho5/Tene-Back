-- Add brand_id column referencing brands table
ALTER TABLE products ADD COLUMN brand_id INTEGER REFERENCES brands(id);

-- Populate brand_id from existing brand name strings
UPDATE products SET brand_id = b.id FROM brands b WHERE products.brand = b.name;

-- Drop old brand varchar column
ALTER TABLE products DROP COLUMN brand;

-- Add index
CREATE INDEX idx_products_brand_id ON products(brand_id);
