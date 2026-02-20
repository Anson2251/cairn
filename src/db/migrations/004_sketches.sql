CREATE TABLE IF NOT EXISTS sketches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    is_public BOOLEAN DEFAULT FALSE,
    deleted_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_sketches_user ON sketches(user_id);
CREATE INDEX idx_sketches_public ON sketches(is_public) WHERE is_public = TRUE;
CREATE INDEX idx_sketches_active ON sketches(id) WHERE deleted_at IS NULL;
