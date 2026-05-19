-- Refresh tokens table for secure token rotation
-- Each refresh token is single-use and tied to a specific user
-- Token is stored hashed to prevent disclosure in DB queries
-- RLS ensures users can only access their own tokens

CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    -- Next token (for rotation chain; helps detect token reuse)
    next_token_hash TEXT,
    -- Family ID groups related tokens (detect token reuse attacks)
    family_id UUID NOT NULL DEFAULT gen_random_uuid(),
    issued_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ,
    invalidated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_token_hash ON refresh_tokens(token_hash);
CREATE INDEX idx_refresh_tokens_family_id ON refresh_tokens(family_id);
CREATE INDEX idx_refresh_tokens_expires_at ON refresh_tokens(expires_at);

-- RLS: Users can only see their own refresh tokens
ALTER TABLE refresh_tokens ENABLE ROW LEVEL SECURITY;

CREATE POLICY refresh_tokens_user_access ON refresh_tokens
    USING (user_id = (current_setting('app.current_user_id', true))::UUID)
    WITH CHECK (user_id = (current_setting('app.current_user_id', true))::UUID);

-- Stored procedure: Issue refresh token on login
-- Returns token (plaintext) only at issuance; stored hashed
CREATE OR REPLACE FUNCTION auth.issue_refresh_token(
    p_user_id UUID,
    p_expiry_hours INT DEFAULT 168 -- 7 days default
)
RETURNS TABLE(
    token TEXT,
    expires_at TIMESTAMPTZ
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
DECLARE
    v_token TEXT;
    v_token_hash TEXT;
    v_expires_at TIMESTAMPTZ;
    v_family_id UUID;
BEGIN
    -- Generate random 32-byte token, encode as hex
    v_token := encode(gen_random_bytes(32), 'hex');
    v_token_hash := 'sha256:' || encode(digest(v_token, 'sha256'), 'hex');
    v_expires_at := NOW() + (p_expiry_hours || ' hours')::INTERVAL;
    v_family_id := gen_random_uuid();

    INSERT INTO refresh_tokens (user_id, token_hash, family_id, expires_at)
    VALUES (p_user_id, v_token_hash, v_family_id, v_expires_at)
    ON CONFLICT DO NOTHING;

    RETURN QUERY SELECT v_token, v_expires_at;
END;
$$;

-- Stored procedure: Refresh access token with rotation
-- Validates token, marks old token as used, issues new token & new refresh token
-- Returns new JWT and rotated refresh token
CREATE OR REPLACE FUNCTION auth.refresh_access_token(
    p_token_hash TEXT,
    p_user_id UUID
)
RETURNS TABLE(
    success BOOLEAN,
    message TEXT,
    new_token TEXT,
    new_refresh_token TEXT,
    expires_at TIMESTAMPTZ
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
DECLARE
    v_token_record RECORD;
    v_new_refresh_token TEXT;
    v_new_expires_at TIMESTAMPTZ;
    v_family_id UUID;
BEGIN
    PERFORM set_config('app.auth_context', 'true', true);

    -- Find valid refresh token
    SELECT rt.* INTO v_token_record
    FROM refresh_tokens rt
    WHERE rt.token_hash = p_token_hash
        AND rt.user_id = p_user_id
        AND rt.expires_at > NOW()
        AND rt.invalidated_at IS NULL
    LIMIT 1;

    IF NOT FOUND THEN
        -- Token not found, expired, or invalid
        RETURN QUERY SELECT false, 'Invalid or expired refresh token'::TEXT, NULL::TEXT, NULL::TEXT, NULL::TIMESTAMPTZ;
        RETURN;
    END IF;

    v_family_id := v_token_record.family_id;

    -- Mark old token as used and update
    UPDATE refresh_tokens
    SET last_used_at = NOW(),
        updated_at = NOW()
    WHERE id = v_token_record.id;

    -- Issue new refresh token (rotated)
    SELECT rt.token, rt.expires_at
    INTO v_new_refresh_token, v_new_expires_at
    FROM auth.issue_refresh_token(p_user_id, 168);

    -- Mark old token's family as having been rotated (next_token_hash for chain validation)
    UPDATE refresh_tokens
    SET next_token_hash = 'sha256:' || encode(digest(v_new_refresh_token, 'sha256'), 'hex')
    WHERE id = v_token_record.id;

    -- Return success with new tokens
    RETURN QUERY SELECT 
        true,
        'Token refreshed successfully'::TEXT,
        NULL::TEXT,  -- New JWT will be generated in app layer
        v_new_refresh_token,
        v_new_expires_at;
END;
$$;

-- Stored procedure: Invalidate token family on suspicious activity
-- Call if same token is used twice (indicates replay attack)
CREATE OR REPLACE FUNCTION auth.invalidate_token_family(
    p_family_id UUID
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
BEGIN
    UPDATE refresh_tokens
    SET invalidated_at = NOW(),
        updated_at = NOW()
    WHERE family_id = p_family_id
        AND invalidated_at IS NULL;
END;
$$;

-- Background job: Clean up expired tokens weekly
CREATE OR REPLACE FUNCTION auth.cleanup_expired_refresh_tokens()
RETURNS TABLE(deleted_count INT)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
DECLARE
    v_deleted INT;
BEGIN
    DELETE FROM refresh_tokens
    WHERE expires_at < NOW()
        OR (invalidated_at IS NOT NULL AND updated_at < NOW() - INTERVAL '30 days');

    GET DIAGNOSTICS v_deleted = ROW_COUNT;
    RETURN QUERY SELECT v_deleted;
END;
$$;
