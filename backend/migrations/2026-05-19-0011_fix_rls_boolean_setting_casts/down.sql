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

DROP POLICY IF EXISTS oauth_accounts_access_policy ON oauth_accounts;

CREATE POLICY oauth_accounts_access_policy ON oauth_accounts
    FOR ALL
    USING (
        (current_setting('app.current_user_id', true) != ''
         AND user_id = current_setting('app.current_user_id', true)::UUID)
        OR COALESCE(current_setting('app.auth_context', true)::BOOLEAN, false)
    )
    WITH CHECK (
        (current_setting('app.current_user_id', true) != ''
         AND user_id = current_setting('app.current_user_id', true)::UUID)
        OR COALESCE(current_setting('app.auth_context', true)::BOOLEAN, false)
    );
