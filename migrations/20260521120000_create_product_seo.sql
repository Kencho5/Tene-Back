CREATE TABLE product_seo (
    product_id       TEXT PRIMARY KEY REFERENCES products(id) ON DELETE CASCADE,
    meta_title       TEXT,
    meta_description TEXT,
    meta_keywords    TEXT[] NOT NULL DEFAULT '{}',
    slug             TEXT UNIQUE,
    search_terms     TEXT[] NOT NULL DEFAULT '{}',
    faqs             JSONB NOT NULL DEFAULT '[]'::jsonb,
    og_image_uuid    UUID,
    no_index         BOOLEAN NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_product_seo_slug ON product_seo(slug);
