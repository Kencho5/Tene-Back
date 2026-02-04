-- Create category_images table
CREATE TABLE category_images (
    category_id INTEGER NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    image_uuid UUID NOT NULL,
    extension VARCHAR(10) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    PRIMARY KEY (category_id, image_uuid)
);

CREATE INDEX idx_category_images_category_id ON category_images(category_id);
CREATE INDEX idx_category_images_image_uuid ON category_images(image_uuid);
