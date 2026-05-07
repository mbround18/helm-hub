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
        created_at -> Text,
        updated_at -> Text,
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

diesel::joinable!(charts -> users (owner_id));
diesel::joinable!(chart_versions -> charts (chart_id));

diesel::allow_tables_to_appear_in_same_query!(users, charts, chart_versions,);
