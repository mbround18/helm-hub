-- Fix RLS policies that cast current_setting(..., true) directly to BOOLEAN.
-- When the setting is unset PostgreSQL can return an empty string, and
-- ''::BOOLEAN raises: invalid input syntax for type boolean: "".

DROP POLICY IF EXISTS users_access_policy ON users;

CREATE POLICY users_access_policy ON users
    FOR ALL
    USING (
        (current_setting('app.current_user_id', true) != ''
         AND id = current_setting('app.current_user_id', true)::UUID)
        OR COALESCE(NULLIF(current_setting('app.is_admin', true), '')::BOOLEAN, false)
        OR COALESCE(NULLIF(current_setting('app.auth_context', true), '')::BOOLEAN, false)
    )
    WITH CHECK (
        (current_setting('app.current_user_id', true) != ''
         AND id = current_setting('app.current_user_id', true)::UUID)
        OR COALESCE(NULLIF(current_setting('app.is_admin', true), '')::BOOLEAN, false)
        OR COALESCE(NULLIF(current_setting('app.auth_context', true), '')::BOOLEAN, false)
    );

DROP POLICY IF EXISTS oauth_accounts_access_policy ON oauth_accounts;

CREATE POLICY oauth_accounts_access_policy ON oauth_accounts
    FOR ALL
    USING (
        (current_setting('app.current_user_id', true) != ''
         AND user_id = current_setting('app.current_user_id', true)::UUID)
        OR COALESCE(NULLIF(current_setting('app.auth_context', true), '')::BOOLEAN, false)
    )
    WITH CHECK (
        (current_setting('app.current_user_id', true) != ''
         AND user_id = current_setting('app.current_user_id', true)::UUID)
        OR COALESCE(NULLIF(current_setting('app.auth_context', true), '')::BOOLEAN, false)
    );
