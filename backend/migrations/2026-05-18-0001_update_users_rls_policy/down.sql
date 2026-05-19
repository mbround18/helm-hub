DROP POLICY IF EXISTS users_access_policy ON users;

CREATE POLICY users_self_policy ON users
    FOR ALL
    USING (id = current_setting('app.current_user_id', true)::UUID);
