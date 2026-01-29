-- Add extension column to product_images
ALTER TABLE product_images ADD COLUMN extension VARCHAR(10) NOT NULL DEFAULT 'jpg';
