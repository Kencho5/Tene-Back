ALTER TABLE products
    ADD COLUMN videos JSONB NOT NULL DEFAULT '[]'::jsonb;
