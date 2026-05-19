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
