DROP POLICY IF EXISTS users_self_policy ON users;
ALTER TABLE users DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS repositories_owner_policy ON repositories;
ALTER TABLE repositories DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS github_repos_owner_policy ON github_repos;
ALTER TABLE github_repos DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS github_connections_owner_policy ON github_connections;
ALTER TABLE github_connections DISABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS api_tokens_owner_policy ON api_tokens;
ALTER TABLE api_tokens DISABLE ROW LEVEL SECURITY;
