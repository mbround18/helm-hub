ALTER TABLE github_repos
    ADD COLUMN sync_status TEXT NOT NULL DEFAULT 'idle',
    ADD COLUMN sync_started_at TIMESTAMPTZ,
    ADD COLUMN sync_finished_at TIMESTAMPTZ,
    ADD COLUMN sync_error TEXT,
    ADD COLUMN last_sync_report JSONB;
