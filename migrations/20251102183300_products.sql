CREATE TABLE IF NOT EXISTS products (
    id INTEGER PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    price DECIMAL(10, 2) NOT NULL,
    discount DECIMAL(5, 2) DEFAULT 0.00 CHECK (discount >= 0 AND discount <= 100),
    quantity INTEGER DEFAULT 0 CHECK (quantity >= 0),
    specifications JSONB DEFAULT '{}'::JSONB,
    product_type TEXT NOT NULL,
    brand VARCHAR(255),
    warranty VARCHAR(50),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_products_created_at ON products(created_at DESC);
CREATE INDEX idx_products_price ON products(price);
CREATE INDEX idx_products_name ON products(name);
CREATE INDEX idx_products_discount ON products(discount DESC);
CREATE INDEX idx_products_quantity ON products(quantity);
CREATE INDEX idx_products_specifications ON products USING GIN (specifications);
CREATE INDEX idx_products_product_type ON products(product_type);
CREATE INDEX idx_products_brand ON products(brand);
CREATE INDEX idx_products_warranty ON products(warranty);
