CREATE TABLE artifacts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    type TEXT NOT NULL, -- e.g., 'helm', 'oci'
    description TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    is_private BOOLEAN NOT NULL DEFAULT FALSE,
    download_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id, name)
);

CREATE INDEX idx_artifacts_name_trgm ON artifacts USING gin (name gin_trgm_ops);
CREATE INDEX idx_artifacts_metadata_gin ON artifacts USING gin (metadata);

-- Row Level Security (RLS) example for Artifacts
ALTER TABLE artifacts ENABLE ROW LEVEL SECURITY;

CREATE POLICY artifact_access_policy ON artifacts
    FOR ALL
    USING (
        is_private = FALSE
        OR owner_id = current_setting('app.current_user_id', true)::UUID
    );
