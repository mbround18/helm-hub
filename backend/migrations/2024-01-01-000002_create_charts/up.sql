CREATE TABLE charts (
    id          TEXT PRIMARY KEY NOT NULL,          -- UUID v4
    owner_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,                      -- chart name, e.g. "my-app"
    description TEXT,
    home_url    TEXT,
    icon_url    TEXT,
    keywords    TEXT,                               -- JSON array stored as text
    is_private  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,

    UNIQUE(owner_id, name)
);

CREATE INDEX idx_charts_owner_id ON charts(owner_id);
CREATE INDEX idx_charts_name     ON charts(name);
