CREATE TABLE downloads (
    id UUID NOT NULL DEFAULT uuid_generate_v4(),
    artifact_version_id UUID NOT NULL REFERENCES artifact_versions(id) ON DELETE CASCADE,
    ip INET,
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id, created_at)
) PARTITION BY RANGE (created_at);

CREATE TABLE downloads_y2026m05 PARTITION OF downloads FOR VALUES FROM ('2026-05-01') TO ('2026-06-01');
