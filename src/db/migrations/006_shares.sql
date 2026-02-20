CREATE TABLE IF NOT EXISTS shares (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sketch_id UUID NOT NULL REFERENCES sketches(id),
    user_id UUID NOT NULL REFERENCES users(id),
    access_level VARCHAR(20) NOT NULL DEFAULT 'view',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID NOT NULL REFERENCES users(id),

    UNIQUE(sketch_id, user_id)
);

CREATE INDEX idx_shares_user ON shares(user_id);
