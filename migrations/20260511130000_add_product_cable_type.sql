ALTER TABLE products
    ADD COLUMN cable_type_id INTEGER REFERENCES cable_types(id) ON DELETE SET NULL;

CREATE INDEX idx_products_cable_type_id ON products(cable_type_id);
