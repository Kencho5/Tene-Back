CREATE TABLE IF NOT EXISTS cable_specs (
    id SERIAL PRIMARY KEY,
    cable_type VARCHAR(50) NOT NULL,
    watts INTEGER NOT NULL CHECK (watts > 0),
    length_cm INTEGER NOT NULL CHECK (length_cm > 0),
    price DECIMAL(10, 2) NOT NULL CHECK (price >= 0),
    warranty_months INTEGER NOT NULL CHECK (warranty_months >= 0),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE (cable_type, watts, length_cm)
);

CREATE INDEX idx_cable_specs_cable_type ON cable_specs(cable_type);
CREATE INDEX idx_cable_specs_length_cm ON cable_specs(length_cm);
