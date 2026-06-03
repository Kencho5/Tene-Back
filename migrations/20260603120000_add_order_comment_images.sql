CREATE TABLE order_comment_images (
    id SERIAL PRIMARY KEY,
    order_id INTEGER REFERENCES orders(id) ON DELETE CASCADE,
    image_uuid UUID NOT NULL UNIQUE,
    extension VARCHAR(10) NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_order_comment_images_order_id ON order_comment_images(order_id);
