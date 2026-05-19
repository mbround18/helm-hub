CREATE TABLE oauth_accounts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    provider TEXT NOT NULL,
    provider_account_id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email TEXT,
    display_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(provider, provider_account_id),
    UNIQUE(provider, user_id)
);

ALTER TABLE oauth_accounts ENABLE ROW LEVEL SECURITY;
ALTER TABLE oauth_accounts FORCE ROW LEVEL SECURITY;

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
