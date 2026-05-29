CREATE TABLE checkout_analytics (
    id               BIGSERIAL PRIMARY KEY,
    session_id       UUID NOT NULL,
    type             VARCHAR(30) NOT NULL,
    step             VARCHAR(30),
    step_index       INTEGER,
    field            VARCHAR(60),
    value            TEXT,
    order_id         VARCHAR(255),
    is_guest         BOOLEAN,
    user_id          INTEGER REFERENCES users(id) ON DELETE SET NULL,
    client_timestamp BIGINT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_checkout_analytics_session ON checkout_analytics(session_id);
CREATE INDEX idx_checkout_analytics_order ON checkout_analytics(order_id);
