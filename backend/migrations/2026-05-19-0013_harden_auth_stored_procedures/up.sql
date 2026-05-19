DROP FUNCTION IF EXISTS auth.register_user(TEXT, TEXT, TEXT);
DROP FUNCTION IF EXISTS auth.login_user(TEXT);

CREATE OR REPLACE FUNCTION auth.register_user(
    p_username TEXT,
    p_email TEXT,
    p_password_hash TEXT,
    p_admin_username TEXT DEFAULT NULL
)
RETURNS users
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
DECLARE
    new_user users%ROWTYPE;
    bootstrap_owner BOOLEAN := false;
BEGIN
    PERFORM set_config('app.auth_context', 'true', true);

    IF NULLIF(BTRIM(COALESCE(p_admin_username, '')), '') IS NOT NULL THEN
        bootstrap_owner := LOWER(BTRIM(p_username)) = LOWER(BTRIM(p_admin_username));
    END IF;

    INSERT INTO users (
        username,
        email,
        password_hash,
        totp_secret,
        totp_enabled,
        is_admin,
        role,
        banned_at,
        storage_usage_bytes,
        storage_quota_bytes
    )
    VALUES (
        p_username,
        p_email,
        p_password_hash,
        NULL,
        false,
        bootstrap_owner,
        CASE
            WHEN bootstrap_owner THEN 'Owner'::user_role
            ELSE 'User'::user_role
        END,
        NULL,
        0,
        NULL
    )
    RETURNING * INTO new_user;

    RETURN new_user;
END;
$$;

CREATE OR REPLACE FUNCTION auth.login_user(
    p_username TEXT,
    p_admin_username TEXT DEFAULT NULL
)
RETURNS SETOF users
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
DECLARE
    existing_user users%ROWTYPE;
    bootstrap_owner BOOLEAN := false;
BEGIN
    PERFORM set_config('app.auth_context', 'true', true);

    SELECT *
    INTO existing_user
    FROM users
    WHERE username = p_username
    LIMIT 1;

    IF NOT FOUND THEN
        RETURN;
    END IF;

    IF NULLIF(BTRIM(COALESCE(p_admin_username, '')), '') IS NOT NULL THEN
        bootstrap_owner := LOWER(BTRIM(existing_user.username)) = LOWER(BTRIM(p_admin_username));
    END IF;

    IF bootstrap_owner THEN
        IF existing_user.role <> 'Owner'::user_role
           OR existing_user.is_admin IS DISTINCT FROM true
           OR (existing_user.totp_enabled AND existing_user.totp_secret IS NULL)
           OR existing_user.storage_usage_bytes < 0 THEN
            UPDATE users
            SET role = 'Owner'::user_role,
                is_admin = true,
                totp_enabled = CASE WHEN totp_secret IS NULL THEN false ELSE totp_enabled END,
                storage_usage_bytes = GREATEST(storage_usage_bytes, 0),
                updated_at = NOW()
            WHERE id = existing_user.id
            RETURNING * INTO existing_user;
        END IF;
    ELSE
        IF (existing_user.role = 'User'::user_role AND existing_user.is_admin)
           OR (existing_user.role IN ('Admin'::user_role, 'Owner'::user_role) AND NOT existing_user.is_admin)
           OR (existing_user.totp_enabled AND existing_user.totp_secret IS NULL)
           OR existing_user.storage_usage_bytes < 0 THEN
            UPDATE users
            SET role = CASE
                    WHEN is_admin AND role = 'User'::user_role THEN 'Admin'::user_role
                    ELSE role
                END,
                is_admin = CASE
                    WHEN role IN ('Admin'::user_role, 'Owner'::user_role) THEN true
                    ELSE is_admin
                END,
                totp_enabled = CASE WHEN totp_secret IS NULL THEN false ELSE totp_enabled END,
                storage_usage_bytes = GREATEST(storage_usage_bytes, 0),
                updated_at = NOW()
            WHERE id = existing_user.id
            RETURNING * INTO existing_user;
        END IF;
    END IF;

    RETURN NEXT existing_user;
END;
$$;
