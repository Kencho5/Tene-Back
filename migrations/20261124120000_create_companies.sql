CREATE TABLE companies (
    id                     SERIAL PRIMARY KEY,
    name                   VARCHAR(255) NOT NULL,
    identification_code    VARCHAR(32) NOT NULL UNIQUE,
    company_type           TEXT NOT NULL CHECK (company_type IN ('legal', 'individual')),
    is_active              BOOLEAN NOT NULL DEFAULT TRUE,
    payment_terms_days     INTEGER NOT NULL DEFAULT 15 CHECK (payment_terms_days >= 0),
    payment_terms          TEXT,
    requires_confirmation  BOOLEAN NOT NULL DEFAULT TRUE,
    notes                  TEXT,
    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_companies_name_trgm ON companies USING gin (name gin_trgm_ops);

CREATE TABLE company_addresses (
    id          SERIAL PRIMARY KEY,
    company_id  INTEGER NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    label       VARCHAR(255),
    city        VARCHAR(100),
    address     TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_company_addresses_company_id ON company_addresses(company_id);

CREATE TABLE company_contacts (
    id                 SERIAL PRIMARY KEY,
    company_id         INTEGER NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    full_name          VARCHAR(255) NOT NULL,
    phone              VARCHAR(32),
    email              VARCHAR(255),
    receives_invoices  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (NOT receives_invoices OR email IS NOT NULL)
);

CREATE INDEX idx_company_contacts_company_id ON company_contacts(company_id);

CREATE TABLE company_contracts (
    id                  SERIAL PRIMARY KEY,
    company_id          INTEGER NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    contract_number     VARCHAR(100),
    start_date          DATE NOT NULL,
    end_date            DATE NOT NULL,
    terms               TEXT,
    file_uuid           UUID,
    file_extension      VARCHAR(10),
    file_name           VARCHAR(255),
    expiry_notified_at  TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (end_date >= start_date)
);

CREATE INDEX idx_company_contracts_company_id ON company_contracts(company_id);
CREATE INDEX idx_company_contracts_end_date ON company_contracts(end_date);

CREATE TABLE company_tariffs (
    id              SERIAL PRIMARY KEY,
    company_id      INTEGER NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    waste_type      TEXT NOT NULL CHECK (waste_type IN ('plastic', 'paper', 'aluminum')),
    price_per_kg    NUMERIC(10, 2) NOT NULL CHECK (price_per_kg >= 0),
    fixed_fee       NUMERIC(10, 2) NOT NULL DEFAULT 0 CHECK (fixed_fee >= 0),
    effective_from  DATE NOT NULL,
    note            TEXT,
    created_by      INTEGER REFERENCES users(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (company_id, waste_type, effective_from)
);

CREATE INDEX idx_company_tariffs_lookup ON company_tariffs(company_id, waste_type, effective_from DESC);

CREATE SEQUENCE company_invoice_number_seq;

CREATE TABLE company_pickups (
    id                SERIAL PRIMARY KEY,
    company_id        INTEGER NOT NULL REFERENCES companies(id) ON DELETE RESTRICT,
    address_id        INTEGER REFERENCES company_addresses(id) ON DELETE SET NULL,
    address_snapshot  TEXT,
    pickup_date       DATE NOT NULL,
    waste_type        TEXT NOT NULL CHECK (waste_type IN ('plastic', 'paper', 'aluminum')),
    quantity_kg       NUMERIC(12, 2) NOT NULL CHECK (quantity_kg > 0),
    tariff_id         INTEGER REFERENCES company_tariffs(id) ON DELETE SET NULL,
    unit_price        NUMERIC(10, 2),
    fixed_fee         NUMERIC(10, 2) NOT NULL DEFAULT 0,
    amount            NUMERIC(12, 2) NOT NULL CHECK (amount >= 0),
    amount_overridden BOOLEAN NOT NULL DEFAULT FALSE,
    status            TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'confirmed')),
    notes             TEXT,
    created_by        INTEGER REFERENCES users(id) ON DELETE SET NULL,
    confirmed_by      INTEGER REFERENCES users(id) ON DELETE SET NULL,
    confirmed_at      TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_company_pickups_company_date ON company_pickups(company_id, pickup_date DESC);
CREATE INDEX idx_company_pickups_date ON company_pickups(pickup_date DESC);
CREATE INDEX idx_company_pickups_status ON company_pickups(status);

CREATE TABLE company_invoices (
    id              SERIAL PRIMARY KEY,
    invoice_number  VARCHAR(32) NOT NULL UNIQUE DEFAULT (
                        'INV-' || to_char(NOW(), 'YYYY') || '-' ||
                        lpad(nextval('company_invoice_number_seq')::text, 6, '0')
                    ),
    company_id      INTEGER NOT NULL REFERENCES companies(id) ON DELETE RESTRICT,
    pickup_id       INTEGER NOT NULL UNIQUE REFERENCES company_pickups(id) ON DELETE RESTRICT,
    issue_date      DATE NOT NULL,
    due_date        DATE NOT NULL,
    total           NUMERIC(12, 2) NOT NULL,
    html            TEXT NOT NULL,
    paid_at         TIMESTAMPTZ,
    last_sent_at    TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_company_invoices_company_id ON company_invoices(company_id);
CREATE INDEX idx_company_invoices_issue_date ON company_invoices(issue_date DESC);

CREATE TABLE company_invoice_deliveries (
    id          SERIAL PRIMARY KEY,
    invoice_id  INTEGER NOT NULL REFERENCES company_invoices(id) ON DELETE CASCADE,
    recipients  TEXT[] NOT NULL,
    success     BOOLEAN NOT NULL,
    error       TEXT,
    sent_by     INTEGER REFERENCES users(id) ON DELETE SET NULL,
    sent_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_company_invoice_deliveries_invoice_id ON company_invoice_deliveries(invoice_id);
