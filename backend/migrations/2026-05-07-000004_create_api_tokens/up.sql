CREATE TABLE api_tokens (
    id           TEXT PRIMARY KEY NOT NULL,      -- UUID v4
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    description  TEXT NOT NULL,                 -- user-supplied label
    token_hash   TEXT NOT NULL UNIQUE,          -- SHA-256 hex of the raw token
    expires_at   TEXT NOT NULL,                 -- ISO-8601 UTC
    last_used_at TEXT,                          -- ISO-8601 UTC, NULL until first use
    created_at   TEXT NOT NULL
);

CREATE INDEX idx_api_tokens_user_id    ON api_tokens(user_id);
CREATE INDEX idx_api_tokens_token_hash ON api_tokens(token_hash);
