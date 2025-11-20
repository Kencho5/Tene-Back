CREATE TABLE product_images (
    id            SERIAL PRIMARY KEY,
    product_id    INTEGER REFERENCES products(id) ON DELETE CASCADE,
    image_uuid    UUID                     NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    color         TEXT,                               
    is_primary    BOOLEAN                  DEFAULT FALSE,
    created_at    TIMESTAMPTZ              DEFAULT NOW()
);

CREATE INDEX idx_product_images_product_id ON product_images(product_id);
