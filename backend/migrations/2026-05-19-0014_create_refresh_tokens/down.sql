-- Down: Drop refresh tokens table and related functions
DROP FUNCTION IF EXISTS auth.cleanup_expired_refresh_tokens();
DROP FUNCTION IF EXISTS auth.invalidate_token_family(UUID);
DROP FUNCTION IF EXISTS auth.refresh_access_token(TEXT, UUID);
DROP FUNCTION IF EXISTS auth.issue_refresh_token(UUID, INT);

DROP TABLE IF EXISTS refresh_tokens;
