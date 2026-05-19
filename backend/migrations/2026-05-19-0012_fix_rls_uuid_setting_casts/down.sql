DROP POLICY IF EXISTS artifact_access_policy ON artifacts;
CREATE POLICY artifact_access_policy ON artifacts
    FOR ALL
    USING (
        is_private = FALSE
        OR (current_setting('app.current_user_id', true) != ''
            AND owner_id = current_setting('app.current_user_id', true)::UUID)
    );

DROP POLICY IF EXISTS api_tokens_owner_policy ON api_tokens;
CREATE POLICY api_tokens_owner_policy ON api_tokens
    FOR ALL
    USING (current_setting('app.current_user_id', true) != ''
           AND user_id = current_setting('app.current_user_id', true)::UUID);

DROP POLICY IF EXISTS github_connections_owner_policy ON github_connections;
CREATE POLICY github_connections_owner_policy ON github_connections
    FOR ALL
    USING (current_setting('app.current_user_id', true) != ''
           AND user_id = current_setting('app.current_user_id', true)::UUID);

DROP POLICY IF EXISTS github_repos_owner_policy ON github_repos;
CREATE POLICY github_repos_owner_policy ON github_repos
    FOR ALL
    USING (current_setting('app.current_user_id', true) != ''
           AND user_id = current_setting('app.current_user_id', true)::UUID);

DROP POLICY IF EXISTS repositories_owner_policy ON repositories;
CREATE POLICY repositories_owner_policy ON repositories
    FOR ALL
    USING (current_setting('app.current_user_id', true) != ''
           AND user_id = current_setting('app.current_user_id', true)::UUID);

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
