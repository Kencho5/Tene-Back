ALTER TABLE orders
    ADD COLUMN source VARCHAR(20) NOT NULL DEFAULT 'web',
    ADD COLUMN created_by_user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN payment_method VARCHAR(20);

CREATE INDEX idx_orders_source ON orders(source);
CREATE INDEX idx_orders_created_by_user_id ON orders(created_by_user_id);
