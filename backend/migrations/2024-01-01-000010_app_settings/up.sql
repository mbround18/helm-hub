CREATE TABLE app_settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Defaults — overridable by admins at runtime.
INSERT INTO app_settings (key, value, updated_at) VALUES
    ('signup_enabled', 'true',     datetime('now')),
    ('app_name',       'Helm Hub', datetime('now')),
    ('logo_url',       '',         datetime('now'));
