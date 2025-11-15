CREATE TABLE IF NOT EXISTS email_verification_codes (
  id SERIAL PRIMARY KEY,
  email VARCHAR(255) NOT NULL,
  code INTEGER NOT NULL,
  expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_email_verification_email ON email_verification_codes(email);
CREATE INDEX idx_email_verification_expires ON email_verification_codes(expires_at);
