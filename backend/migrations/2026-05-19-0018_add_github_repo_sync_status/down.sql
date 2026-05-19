ALTER TABLE github_repos
    DROP COLUMN IF EXISTS last_sync_report,
    DROP COLUMN IF EXISTS sync_error,
    DROP COLUMN IF EXISTS sync_finished_at,
    DROP COLUMN IF EXISTS sync_started_at,
    DROP COLUMN IF EXISTS sync_status;
