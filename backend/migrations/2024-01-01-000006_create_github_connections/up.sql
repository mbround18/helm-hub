CREATE TABLE github_connections (
    id                   TEXT PRIMARY KEY NOT NULL,
    user_id              TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Numeric GitHub user ID — stable even if the username changes.
    github_id            TEXT NOT NULL UNIQUE,
    github_username      TEXT NOT NULL,
    -- OAuth access token.  Stored in plaintext; user can revoke via GitHub settings.
    github_access_token  TEXT NOT NULL,
    avatar_url           TEXT,
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
);

CREATE INDEX idx_github_connections_user_id ON github_connections (user_id);
