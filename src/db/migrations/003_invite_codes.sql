CREATE TABLE IF NOT EXISTS invite_codes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sequence INTEGER UNIQUE NOT NULL,
    code VARCHAR(64) UNIQUE NOT NULL,
    cairn_name VARCHAR(64) NOT NULL,
    origin_coord POINT,
    memo TEXT,
    used BOOLEAN DEFAULT FALSE,
    used_by UUID REFERENCES users(id),
    used_at TIMESTAMP WITH TIME ZONE,
    expires_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_invite_code ON invite_codes(code);
