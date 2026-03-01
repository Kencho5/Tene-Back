-- Function to sync products.quantity with sum of product_images.quantity
CREATE OR REPLACE FUNCTION sync_product_quantity()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE products
    SET quantity = (
        SELECT COALESCE(SUM(quantity), 0)
        FROM product_images
        WHERE product_id = COALESCE(NEW.product_id, OLD.product_id)
    )
    WHERE id = COALESCE(NEW.product_id, OLD.product_id);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger on product_images insert, update, delete
CREATE TRIGGER trg_sync_product_quantity
AFTER INSERT OR UPDATE OF quantity OR DELETE ON product_images
FOR EACH ROW EXECUTE FUNCTION sync_product_quantity();

-- Sync existing data
UPDATE products p
SET quantity = (
    SELECT COALESCE(SUM(pi.quantity), 0)
    FROM product_images pi
    WHERE pi.product_id = p.id
);
