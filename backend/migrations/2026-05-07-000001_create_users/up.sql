CREATE TABLE users (
    id          TEXT PRIMARY KEY NOT NULL,          -- UUID v4
    username    TEXT NOT NULL UNIQUE,
    email       TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,                    -- Argon2id hash
    totp_secret TEXT,                               -- NULL = 2FA disabled
    totp_enabled INTEGER NOT NULL DEFAULT 0,        -- 0=false, 1=true
    is_admin    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,                      -- ISO-8601 UTC
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email    ON users(email);
