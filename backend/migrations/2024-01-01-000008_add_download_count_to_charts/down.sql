-- SQLite does not support DROP COLUMN before 3.35; recreate if needed.
-- For a rollback simply leave the column in place or recreate the table without it.
SELECT 1;
