CREATE TABLE IF NOT EXISTS cable_types (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS cable_variants (
    id SERIAL PRIMARY KEY,
    cable_type_id INTEGER NOT NULL REFERENCES cable_types(id) ON DELETE CASCADE,
    watts INTEGER NOT NULL CHECK (watts > 0),
    length_cm INTEGER NOT NULL CHECK (length_cm > 0),
    price DECIMAL(10, 2) NOT NULL CHECK (price >= 0),
    warranty_months INTEGER NOT NULL CHECK (warranty_months >= 0),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (cable_type_id, watts, length_cm)
);

CREATE INDEX idx_cable_variants_cable_type_id ON cable_variants(cable_type_id);
CREATE INDEX idx_cable_variants_length_cm ON cable_variants(length_cm);
