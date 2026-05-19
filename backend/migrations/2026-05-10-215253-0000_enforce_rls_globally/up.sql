-- Enforce RLS on all user-private tables

-- 1. api_tokens
ALTER TABLE api_tokens ENABLE ROW LEVEL SECURITY;
ALTER TABLE api_tokens FORCE ROW LEVEL SECURITY;
CREATE POLICY api_tokens_owner_policy ON api_tokens
    FOR ALL USING (current_setting('app.current_user_id', true) != '' 
                   AND user_id = current_setting('app.current_user_id', true)::UUID);

-- 2. github_connections
ALTER TABLE github_connections ENABLE ROW LEVEL SECURITY;
ALTER TABLE github_connections FORCE ROW LEVEL SECURITY;
CREATE POLICY github_connections_owner_policy ON github_connections
    FOR ALL USING (current_setting('app.current_user_id', true) != '' 
                   AND user_id = current_setting('app.current_user_id', true)::UUID);

-- 3. github_repos
ALTER TABLE github_repos ENABLE ROW LEVEL SECURITY;
ALTER TABLE github_repos FORCE ROW LEVEL SECURITY;
CREATE POLICY github_repos_owner_policy ON github_repos
    FOR ALL USING (current_setting('app.current_user_id', true) != '' 
                   AND user_id = current_setting('app.current_user_id', true)::UUID);

-- 4. repositories
ALTER TABLE repositories ENABLE ROW LEVEL SECURITY;
ALTER TABLE repositories FORCE ROW LEVEL SECURITY;
CREATE POLICY repositories_owner_policy ON repositories
    FOR ALL USING (current_setting('app.current_user_id', true) != '' 
                   AND user_id = current_setting('app.current_user_id', true)::UUID);

-- 5. users (Users can only see themselves)
ALTER TABLE users ENABLE ROW LEVEL SECURITY;
ALTER TABLE users FORCE ROW LEVEL SECURITY;
CREATE POLICY users_self_policy ON users
    FOR ALL USING (current_setting('app.current_user_id', true) != '' 
                   AND id = current_setting('app.current_user_id', true)::UUID);

-- Note: Admin bypass can be added by checking a 'is_admin' session variable or role.
-- For now, we assume the application handles admin logic or sets the user_id correctly.
