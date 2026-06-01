CREATE TABLE checkout_cart_snapshots (
    session_id  UUID PRIMARY KEY,
    cart        JSONB NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
