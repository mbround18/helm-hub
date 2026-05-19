CREATE OR REPLACE FUNCTION auth.upsert_oauth_user(
    p_provider TEXT,
    p_provider_account_id TEXT,
    p_email TEXT,
    p_display_name TEXT,
    p_password_hash TEXT
)
RETURNS users
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
DECLARE
    existing_user users%ROWTYPE;
    new_user users%ROWTYPE;
    synthesized_email TEXT;
    synthesized_username TEXT;
BEGIN
    PERFORM set_config('app.auth_context', 'true', true);

    SELECT u.*
    INTO existing_user
    FROM oauth_accounts oa
    JOIN users u ON u.id = oa.user_id
    WHERE oa.provider = p_provider
      AND oa.provider_account_id = p_provider_account_id
    LIMIT 1;

    IF FOUND THEN
        RETURN existing_user;
    END IF;

    synthesized_username := substr(
        lower(regexp_replace(p_provider || '-' || p_provider_account_id, '[^a-z0-9-]+', '-', 'g')),
        1,
        39
    );

    synthesized_email := COALESCE(
        NULLIF(p_email, ''),
        synthesized_username || '@' || p_provider || '.local'
    );

    INSERT INTO users (username, email, password_hash)
    VALUES (synthesized_username, synthesized_email, p_password_hash)
    RETURNING * INTO new_user;

    INSERT INTO oauth_accounts (
        provider,
        provider_account_id,
        user_id,
        email,
        display_name
    )
    VALUES (
        p_provider,
        p_provider_account_id,
        new_user.id,
        NULLIF(p_email, ''),
        NULLIF(p_display_name, '')
    );

    RETURN new_user;
END;
$$;
