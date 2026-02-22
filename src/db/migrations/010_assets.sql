CREATE TABLE IF NOT EXISTS assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    original_filename VARCHAR(255),
    mime_type VARCHAR(100) NOT NULL,
    size INTEGER NOT NULL,
    hash VARCHAR(64) NOT NULL,
    data BYTEA NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_assets_owner ON assets(owner_id);
CREATE INDEX idx_assets_hash ON assets(hash);
CREATE UNIQUE INDEX idx_assets_hash_unique ON assets(hash, owner_id);
