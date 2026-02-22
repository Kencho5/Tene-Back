CREATE TABLE orders (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id),
    order_id VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    payment_id INTEGER,
    amount INTEGER NOT NULL,
    currency VARCHAR(10) NOT NULL DEFAULT 'GEL',
    customer_type VARCHAR(20) NOT NULL,
    customer_name VARCHAR(255),
    customer_surname VARCHAR(255),
    organization_type VARCHAR(50),
    organization_name VARCHAR(255),
    organization_code VARCHAR(100),
    email VARCHAR(255) NOT NULL,
    phone_number BIGINT NOT NULL,
    address TEXT NOT NULL,
    delivery_type VARCHAR(50) NOT NULL,
    delivery_time VARCHAR(50) NOT NULL,
    checkout_url TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE order_items (
    id SERIAL PRIMARY KEY,
    order_id INTEGER NOT NULL REFERENCES orders(id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES products(id),
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    price_at_purchase DECIMAL(10, 2) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_orders_user_id ON orders(user_id);
CREATE INDEX idx_orders_order_id ON orders(order_id);
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_order_items_order_id ON order_items(order_id);
