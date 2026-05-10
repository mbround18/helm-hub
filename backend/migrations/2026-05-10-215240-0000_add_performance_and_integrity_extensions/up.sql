-- Enable citext for case-insensitive logins and emails
CREATE EXTENSION IF NOT EXISTS citext;

-- Enable pg_stat_statements for performance monitoring
-- Note: This requires shared_preload_libraries = 'pg_stat_statements' in postgresql.conf
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

-- Enable pg_buffercache for inspecting the buffer cache
CREATE EXTENSION IF NOT EXISTS pg_buffercache;

-- Convert existing columns to citext
ALTER TABLE users ALTER COLUMN username TYPE citext;
ALTER TABLE users ALTER COLUMN email TYPE citext;
