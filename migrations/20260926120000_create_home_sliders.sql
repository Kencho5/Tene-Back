CREATE TABLE home_sliders (
    id SERIAL PRIMARY KEY,
    title VARCHAR(255),
    link_url TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    display_order INTEGER NOT NULL DEFAULT 0,
    desktop_image_uuid UUID,
    desktop_image_extension VARCHAR(10),
    mobile_image_uuid UUID,
    mobile_image_extension VARCHAR(10),
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_home_sliders_display_order ON home_sliders(display_order, id);
