CREATE TABLE chart_versions (
    id              TEXT PRIMARY KEY NOT NULL,      -- UUID v4
    chart_id        TEXT NOT NULL REFERENCES charts(id) ON DELETE CASCADE,
    version         TEXT NOT NULL,                  -- semver, e.g. "1.2.3"
    app_version     TEXT,                           -- upstream app version
    description     TEXT,
    digest          TEXT NOT NULL,                  -- SHA-256 of .tgz
    storage_path    TEXT NOT NULL,                  -- relative path on disk
    chart_yaml      TEXT NOT NULL,                  -- raw Chart.yaml content
    values_yaml     TEXT,                           -- raw values.yaml content
    schema_json     TEXT,                           -- values.schema.json if present
    deprecated      INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,

    UNIQUE(chart_id, version)
);

CREATE INDEX idx_chart_versions_chart_id ON chart_versions(chart_id);
CREATE INDEX idx_chart_versions_version  ON chart_versions(version);
