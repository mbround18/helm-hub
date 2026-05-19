use serde::Serialize;

use crate::{
    db::DbConn,
    error::AppError,
    services::{settings, token_crypto},
};

pub const KEY_LOCAL_ENABLED: &str = "auth_local_enabled";

pub const KEY_GITHUB_ENABLED: &str = "auth_github_enabled";
pub const KEY_GITHUB_CLIENT_ID: &str = "auth_github_client_id";
pub const KEY_GITHUB_CLIENT_SECRET: &str = "auth_github_client_secret_enc";
pub const KEY_GITHUB_REDIRECT_URI: &str = "auth_github_redirect_uri";

pub const KEY_GITLAB_ENABLED: &str = "auth_gitlab_enabled";
pub const KEY_GITLAB_BASE_URL: &str = "auth_gitlab_base_url";
pub const KEY_GITLAB_CLIENT_ID: &str = "auth_gitlab_client_id";
pub const KEY_GITLAB_CLIENT_SECRET: &str = "auth_gitlab_client_secret_enc";
pub const KEY_GITLAB_REDIRECT_URI: &str = "auth_gitlab_redirect_uri";

// Observability settings
pub const KEY_OTEL_ENDPOINT: &str = "otel_endpoint";
pub const KEY_GRAFANA_URL: &str = "grafana_url";
pub const KEY_GRAFANA_API_TOKEN: &str = "grafana_api_token_enc";

#[derive(Debug, Clone, Serialize)]
pub struct ProviderPublicSettings {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderAdminSettings {
    pub enabled: bool,
    pub client_id: Option<String>,
    pub client_secret_set: bool,
    pub redirect_uri: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthPublicSettings {
    pub local_enabled: bool,
    pub github_enabled: bool,
    pub gitlab_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthAdminSettings {
    pub local_enabled: bool,
    pub github: ProviderAdminSettings,
    pub gitlab: ProviderAdminSettings,
}

pub async fn bool_setting(conn: &mut DbConn, key: &str, default: bool) -> bool {
    match settings::get(conn, key).await {
        Ok(value) => !matches!(value.as_str(), "false" | "0" | "off"),
        Err(_) => default,
    }
}

pub async fn set_bool(conn: &mut DbConn, key: &str, value: bool) -> Result<(), AppError> {
    settings::set(conn, key, if value { "true" } else { "false" }).await
}

pub async fn opt_setting(conn: &mut DbConn, key: &str) -> Option<String> {
    settings::get(conn, key).await.ok()
}

pub async fn set_opt_setting(
    conn: &mut DbConn,
    key: &str,
    value: Option<&str>,
) -> Result<(), AppError> {
    if let Some(value) = value {
        settings::set(conn, key, value).await?;
    }
    Ok(())
}

pub async fn get_secret(
    conn: &mut DbConn,
    key: &str,
    crypto_key: &[u8; 32],
) -> Result<Option<String>, AppError> {
    match settings::get(conn, key).await {
        Ok(value) => Ok(Some(token_crypto::decrypt_token(&value, crypto_key)?)),
        Err(_) => Ok(None),
    }
}

pub async fn set_secret(
    conn: &mut DbConn,
    key: &str,
    value: Option<&str>,
    crypto_key: &[u8; 32],
) -> Result<(), AppError> {
    if let Some(value) = value {
        let encrypted = token_crypto::encrypt_token(value, crypto_key)?;
        settings::set(conn, key, &encrypted).await?;
    }
    Ok(())
}
