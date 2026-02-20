CREATE TABLE IF NOT EXISTS routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sketch_id UUID NOT NULL REFERENCES sketches(id),
    name VARCHAR(255),
    description TEXT,
    geojson JSONB NOT NULL,
    metadata JSONB DEFAULT '{}',
    notes TEXT,
    version INTEGER NOT NULL DEFAULT 1,
    deleted_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

CREATE INDEX idx_routes_sketch ON routes(sketch_id);
CREATE INDEX idx_routes_active ON routes(id) WHERE deleted_at IS NULL;
