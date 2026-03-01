ALTER TABLE product_images ADD COLUMN quantity INTEGER NOT NULL DEFAULT 0 CHECK (quantity >= 0);

-- Set quantity to 1 if product has stock, 0 if not
UPDATE product_images pi
SET quantity = CASE WHEN p.quantity > 0 THEN 1 ELSE 0 END
FROM products p
WHERE pi.product_id = p.id;
