// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;

    api_tokens (id) {
        id -> Uuid,
        user_id -> Uuid,
        description -> Text,
        token_hash -> Text,
        expires_at -> Timestamptz,
        last_used_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    app_settings (key) {
        key -> Text,
        value -> Text,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    artifact_versions (id) {
        id -> Uuid,
        artifact_id -> Uuid,
        version -> Text,
        digest -> Bytea,
        size -> BigInt,
        storage_path -> Text,
        metadata -> Jsonb,
        deprecated -> Bool,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    artifacts (id) {
        id -> Uuid,
        owner_id -> Uuid,
        name -> Text,
        r#type -> Text,
        description -> Nullable<Text>,
        metadata -> Jsonb,
        is_private -> Bool,
        download_count -> Integer,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    audit_logs (id, created_at) {
        id -> Uuid,
        actor_id -> Nullable<Uuid>,
        action -> Text,
        target_type -> Text,
        target_id -> Nullable<Uuid>,
        metadata -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    downloads (id, created_at) {
        id -> Uuid,
        artifact_version_id -> Uuid,
        ip -> Nullable<Inet>,
        user_agent -> Nullable<Text>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    github_connections (id) {
        id -> Uuid,
        user_id -> Uuid,
        github_id -> Text,
        github_username -> Text,
        github_access_token -> Text,
        avatar_url -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    github_repos (id) {
        id -> Uuid,
        user_id -> Uuid,
        github_connection_id -> Uuid,
        repo_owner -> Text,
        repo_name -> Text,
        last_synced_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    gitlab_connections (id) {
        id -> Uuid,
        user_id -> Uuid,
        gitlab_id -> Text,
        gitlab_username -> Text,
        gitlab_access_token -> Text,
        avatar_url -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    gitlab_repos (id) {
        id -> Uuid,
        user_id -> Uuid,
        gitlab_connection_id -> Uuid,
        repo_owner -> Text,
        repo_name -> Text,
        last_synced_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    rate_limit_windows (key) {
        key -> Text,
        count -> Integer,
        window_start -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    oauth_accounts (id) {
        id -> Uuid,
        provider -> Text,
        provider_account_id -> Text,
        user_id -> Uuid,
        email -> Nullable<Text>,
        display_name -> Nullable<Text>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    repositories (id) {
        id -> Uuid,
        user_id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
        config -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    use diesel::sql_types::*;

    users (id) {
        id -> Uuid,
        username -> Text,
        email -> Text,
        password_hash -> Text,
        totp_secret -> Nullable<Text>,
        totp_enabled -> Bool,
        is_admin -> Bool,
        role -> Text,
        banned_at -> Nullable<Timestamptz>,
        storage_usage_bytes -> BigInt,
        storage_quota_bytes -> Nullable<BigInt>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::joinable!(api_tokens -> users (user_id));
diesel::joinable!(artifact_versions -> artifacts (artifact_id));
diesel::joinable!(artifacts -> users (owner_id));
diesel::joinable!(audit_logs -> users (actor_id));
diesel::joinable!(downloads -> artifact_versions (artifact_version_id));
diesel::joinable!(github_connections -> users (user_id));
diesel::joinable!(github_repos -> github_connections (github_connection_id));
diesel::joinable!(github_repos -> users (user_id));
diesel::joinable!(gitlab_connections -> users (user_id));
diesel::joinable!(gitlab_repos -> gitlab_connections (gitlab_connection_id));
diesel::joinable!(gitlab_repos -> users (user_id));
diesel::joinable!(oauth_accounts -> users (user_id));
diesel::joinable!(repositories -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    api_tokens,
    app_settings,
    artifact_versions,
    artifacts,
    audit_logs,
    downloads,
    github_connections,
    github_repos,
    gitlab_connections,
    gitlab_repos,
    oauth_accounts,
    rate_limit_windows,
    repositories,
    users,
);
