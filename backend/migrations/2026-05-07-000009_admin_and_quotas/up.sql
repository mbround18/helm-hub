-- Storage quota tracking on users
ALTER TABLE users ADD COLUMN banned_at          TEXT;
ALTER TABLE users ADD COLUMN storage_usage_bytes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN storage_quota_bytes INTEGER;          -- NULL = use global default

-- Admin action audit trail
CREATE TABLE admin_audit_log (
    id           TEXT PRIMARY KEY NOT NULL,
    admin_id     TEXT NOT NULL,
    action       TEXT NOT NULL,
    target_type  TEXT NOT NULL,
    target_id    TEXT NOT NULL,
    metadata_json TEXT,
    created_at   TEXT NOT NULL
);

CREATE INDEX idx_audit_admin_id  ON admin_audit_log(admin_id);
CREATE INDEX idx_audit_created   ON admin_audit_log(created_at);
