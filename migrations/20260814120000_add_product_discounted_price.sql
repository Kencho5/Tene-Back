ALTER TABLE products
ADD COLUMN discounted_price DECIMAL(10, 2);

UPDATE products
SET discounted_price = ROUND(price * (1 - discount / 100), 2)
WHERE discount > 0;
