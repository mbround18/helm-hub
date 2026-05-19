//! Observability configuration and helpers for OTEL and Grafana integration.

use crate::{
    db::DbConn,
    error::AppError,
    services::auth_providers,
};

/// Retrieves the OTEL endpoint from app_settings, falling back to env var.
pub async fn otel_endpoint(conn: &mut DbConn) -> Option<String> {
    // Try to get from app_settings first
    if let Some(endpoint) = auth_providers::opt_setting(conn, auth_providers::KEY_OTEL_ENDPOINT).await
        && !endpoint.is_empty()
    {
        return Some(endpoint);
    }
    
    // Fall back to env var
    std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok()
}

#[derive(Debug, Clone)]
pub struct GrafanaConfig {
    pub url: String,
    pub api_token: String,
}

/// Retrieves Grafana configuration from app_settings if both URL and token are set.
pub async fn grafana_config(
    conn: &mut DbConn,
    encryption_key: &[u8; 32],
) -> Result<Option<GrafanaConfig>, AppError> {
    let url = auth_providers::opt_setting(conn, auth_providers::KEY_GRAFANA_URL).await;
    
    if url.is_none() || url.as_ref().map(|u| u.is_empty()).unwrap_or(true) {
        return Ok(None);
    }
    
    let token_encrypted = auth_providers::opt_setting(conn, auth_providers::KEY_GRAFANA_API_TOKEN).await;
    let token = match token_encrypted {
        Some(enc) => {
            crate::services::token_crypto::decrypt_token(&enc, encryption_key)?
        }
        None => return Ok(None),
    };
    
    if token.is_empty() {
        return Ok(None);
    }
    
    Ok(Some(GrafanaConfig {
        url: url.unwrap(),
        api_token: token,
    }))
}

/// Validates Grafana connectivity by making a test API call.
pub async fn validate_grafana(
    client: &reqwest::Client,
    config: &GrafanaConfig,
) -> Result<(), AppError> {
    let url = format!("{}/api/health", config.url.trim_end_matches('/'));
    
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.api_token))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Grafana health check failed: {e}")))?;
    
    if !response.status().is_success() {
        return Err(AppError::BadRequest(
            format!("Grafana health check returned {}", response.status())
        ));
    }
    
    Ok(())
}

/// Generates a dashboard URL for a chart.
/// 
/// Dashboard naming convention: `helm-hub-{owner}-{chart}`
pub fn dashboard_url(config: &GrafanaConfig, owner: &str, chart: &str) -> String {
    let dashboard_name = format!("helm-hub-{}-{}", owner, chart);
    format!(
        "{}/d/{}?refresh=30s",
        config.url.trim_end_matches('/'),
        dashboard_name
    )
}

/// Returns grafana embed URL for a specific chart panel.
/// 
/// Assumes dashboard and panel exist. Returns URL for embedding.
pub fn embed_panel_url(
    config: &GrafanaConfig,
    owner: &str,
    chart: &str,
) -> String {
    let dashboard_name = format!("helm-hub-{}-{}", owner, chart);
    format!(
        "{}/d/{}?refresh=30s&kiosk=tv",
        config.url.trim_end_matches('/'),
        dashboard_name
    )
}
