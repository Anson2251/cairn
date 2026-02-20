CREATE TABLE IF NOT EXISTS public_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sketch_id UUID NOT NULL REFERENCES sketches(id),
    token VARCHAR(64) UNIQUE NOT NULL,
    access_level VARCHAR(20) NOT NULL DEFAULT 'view',
    expires_at TIMESTAMP WITH TIME ZONE,
    created_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_public_links_token ON public_links(token);
