CREATE TABLE github_repos (
    id                    TEXT PRIMARY KEY NOT NULL,
    user_id               TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    github_connection_id  TEXT NOT NULL REFERENCES github_connections(id) ON DELETE CASCADE,
    repo_owner            TEXT NOT NULL,
    repo_name             TEXT NOT NULL,
    -- ISO-8601 timestamp of the last successful sync, NULL if never synced.
    last_synced_at        TEXT,
    created_at            TEXT NOT NULL
);

-- Prevent the same user from linking the same repo twice.
CREATE UNIQUE INDEX idx_github_repos_unique ON github_repos (user_id, repo_owner, repo_name);
CREATE INDEX idx_github_repos_user_id       ON github_repos (user_id);
