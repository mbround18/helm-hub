DROP FUNCTION IF EXISTS auth.register_user(TEXT, TEXT, TEXT, TEXT);
DROP FUNCTION IF EXISTS auth.login_user(TEXT, TEXT);

CREATE OR REPLACE FUNCTION auth.register_user(
    p_username TEXT,
    p_email TEXT,
    p_password_hash TEXT
)
RETURNS users
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
DECLARE
    new_user users%ROWTYPE;
BEGIN
    PERFORM set_config('app.auth_context', 'true', true);

    INSERT INTO users (username, email, password_hash)
    VALUES (p_username, p_email, p_password_hash)
    RETURNING * INTO new_user;

    RETURN new_user;
END;
$$;

CREATE OR REPLACE FUNCTION auth.login_user(p_username TEXT)
RETURNS SETOF users
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
BEGIN
    PERFORM set_config('app.auth_context', 'true', true);

    RETURN QUERY
    SELECT *
    FROM users
    WHERE username = p_username
    LIMIT 1;
END;
$$;
