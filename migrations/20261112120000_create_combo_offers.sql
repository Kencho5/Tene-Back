CREATE TABLE combo_offers (
    id SERIAL PRIMARY KEY,
    main_product_id TEXT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    pricing_type VARCHAR(20) NOT NULL CHECK (pricing_type IN ('addon_price', 'bundle_price')),
    bundle_price DECIMAL(10, 2) CHECK (bundle_price IS NULL OR bundle_price >= 0),
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    CHECK (pricing_type <> 'bundle_price' OR bundle_price IS NOT NULL)
);

CREATE INDEX idx_combo_offers_main_product_id ON combo_offers(main_product_id);

CREATE TABLE combo_offer_items (
    combo_offer_id INTEGER NOT NULL REFERENCES combo_offers(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    combo_price DECIMAL(10, 2) CHECK (combo_price IS NULL OR combo_price >= 0),
    position INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (combo_offer_id, product_id)
);

CREATE INDEX idx_combo_offer_items_product_id ON combo_offer_items(product_id);

ALTER TABLE order_items
    ADD COLUMN combo_offer_id INTEGER REFERENCES combo_offers(id) ON DELETE SET NULL,
    ADD COLUMN combo_title VARCHAR(255),
    ADD COLUMN combo_group VARCHAR(64),
    ADD COLUMN regular_price DECIMAL(10, 2);
