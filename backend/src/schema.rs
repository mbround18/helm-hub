// @generated automatically by Diesel CLI.

diesel::table! {
    users (id) {
        id -> Text,
        username -> Text,
        email -> Text,
        password_hash -> Text,
        totp_secret -> Nullable<Text>,
        totp_enabled -> Integer,
        is_admin -> Integer,
        banned_at -> Nullable<Text>,
        storage_usage_bytes -> BigInt,
        storage_quota_bytes -> Nullable<BigInt>,
        created_at -> Text,
        updated_at -> Text,
    }
}

diesel::table! {
    admin_audit_log (id) {
        id -> Text,
        admin_id -> Text,
        action -> Text,
        target_type -> Text,
        target_id -> Text,
        metadata_json -> Nullable<Text>,
        created_at -> Text,
    }
}

diesel::table! {
    charts (id) {
        id -> Text,
        owner_id -> Text,
        name -> Text,
        description -> Nullable<Text>,
        home_url -> Nullable<Text>,
        icon_url -> Nullable<Text>,
        keywords -> Nullable<Text>,
        is_private -> Integer,
        created_at -> Text,
        updated_at -> Text,
        download_count -> Integer,
    }
}

diesel::table! {
    chart_versions (id) {
        id -> Text,
        chart_id -> Text,
        version -> Text,
        app_version -> Nullable<Text>,
        description -> Nullable<Text>,
        digest -> Text,
        storage_path -> Text,
        chart_yaml -> Text,
        values_yaml -> Nullable<Text>,
        schema_json -> Nullable<Text>,
        deprecated -> Integer,
        created_at -> Text,
    }
}

diesel::table! {
    api_tokens (id) {
        id -> Text,
        user_id -> Text,
        description -> Text,
        token_hash -> Text,
        expires_at -> Text,
        last_used_at -> Nullable<Text>,
        created_at -> Text,
    }
}

diesel::table! {
    github_connections (id) {
        id -> Text,
        user_id -> Text,
        github_id -> Text,
        github_username -> Text,
        github_access_token -> Text,
        avatar_url -> Nullable<Text>,
        created_at -> Text,
        updated_at -> Text,
    }
}

diesel::table! {
    github_repos (id) {
        id -> Text,
        user_id -> Text,
        github_connection_id -> Text,
        repo_owner -> Text,
        repo_name -> Text,
        last_synced_at -> Nullable<Text>,
        created_at -> Text,
    }
}

diesel::table! {
    rate_limit_windows (key) {
        key -> Text,
        count -> Integer,
        window_start -> Text,
    }
}

diesel::joinable!(charts -> users (owner_id));
diesel::joinable!(chart_versions -> charts (chart_id));
diesel::joinable!(api_tokens -> users (user_id));
diesel::joinable!(github_connections -> users (user_id));
diesel::joinable!(github_repos -> users (user_id));
diesel::joinable!(github_repos -> github_connections (github_connection_id));

diesel::table! {
    app_settings (key) {
        key -> Text,
        value -> Text,
        updated_at -> Text,
    }
}

diesel::joinable!(admin_audit_log -> users (admin_id));

diesel::allow_tables_to_appear_in_same_query!(
    users,
    charts,
    chart_versions,
    api_tokens,
    github_connections,
    github_repos,
    rate_limit_windows,
    admin_audit_log,
    app_settings,
);
