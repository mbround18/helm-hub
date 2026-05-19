DROP POLICY IF EXISTS users_access_policy ON users;

CREATE POLICY users_access_policy ON users
    FOR ALL
    USING (
        id = current_setting('app.current_user_id', true)::UUID
        OR COALESCE(current_setting('app.is_admin', true)::BOOLEAN, false)
        OR username = current_setting('app.login_username', true)
    )
    WITH CHECK (
        id = current_setting('app.current_user_id', true)::UUID
        OR COALESCE(current_setting('app.is_admin', true)::BOOLEAN, false)
        OR username = current_setting('app.login_username', true)
    );

DROP SCHEMA IF EXISTS auth CASCADE;
