CREATE SCHEMA IF NOT EXISTS auth;

DROP POLICY IF EXISTS users_access_policy ON users;

CREATE POLICY users_access_policy ON users
    FOR ALL
    USING (
        (current_setting('app.current_user_id', true) != '' 
         AND id = current_setting('app.current_user_id', true)::UUID)
        OR COALESCE(current_setting('app.is_admin', true)::BOOLEAN, false)
        OR COALESCE(current_setting('app.auth_context', true)::BOOLEAN, false)
    )
    WITH CHECK (
        (current_setting('app.current_user_id', true) != '' 
         AND id = current_setting('app.current_user_id', true)::UUID)
        OR COALESCE(current_setting('app.is_admin', true)::BOOLEAN, false)
        OR COALESCE(current_setting('app.auth_context', true)::BOOLEAN, false)
    );
