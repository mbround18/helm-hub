CREATE TABLE rate_limit_windows (
    -- "ip:<addr>" for anonymous callers, "tok:<16-hex-chars>" for bearer-token callers
    key          TEXT PRIMARY KEY NOT NULL,
    count        INTEGER NOT NULL DEFAULT 1,
    -- Hour-granularity UTC bucket, e.g. "2025-05-07T14"
    window_start TEXT NOT NULL
);

-- Lets us purge stale rows (any maintenance job or future cleanup) efficiently.
CREATE INDEX idx_rate_limit_window_start ON rate_limit_windows (window_start);
