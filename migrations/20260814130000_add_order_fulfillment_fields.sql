ALTER TABLE orders
    ADD COLUMN fulfillment_method VARCHAR(20),
    ADD COLUMN personal_number VARCHAR(50),
    ADD COLUMN source_comment TEXT,
    ADD COLUMN is_installment_sale BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN is_product_exchange BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE orders
    ALTER COLUMN payment_method TYPE VARCHAR(30);
