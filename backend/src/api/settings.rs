use axum::{Json, extract::State};
use serde::Serialize;

use crate::{AppState, error::AppError, services::auth_providers};

#[derive(Serialize)]
pub struct AppSettingsResponse {
    pub app_name: String,
    pub logo_url: String,
    pub signup_enabled: bool,
    pub local_auth_enabled: bool,
    pub github_auth_enabled: bool,
    pub gitlab_auth_enabled: bool,
}

/// `GET /api/settings` — public, no auth required.
pub async fn get_settings(
    State(state): State<AppState>,
) -> Result<Json<AppSettingsResponse>, AppError> {
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;
    let app_name = crate::services::settings::get(&mut conn, "app_name")
        .await
        .unwrap_or_else(|_| "Helm Hub".into());
    let logo_url = crate::services::settings::get(&mut conn, "logo_url")
        .await
        .unwrap_or_default();
    let signup_enabled = crate::services::settings::signup_enabled(&mut conn).await;
    let local_auth_enabled =
        auth_providers::bool_setting(&mut conn, auth_providers::KEY_LOCAL_ENABLED, true).await;
    let github_auth_enabled =
        auth_providers::bool_setting(&mut conn, auth_providers::KEY_GITHUB_ENABLED, false).await;
    let gitlab_auth_enabled =
        auth_providers::bool_setting(&mut conn, auth_providers::KEY_GITLAB_ENABLED, false).await;
    Ok(Json(AppSettingsResponse {
        app_name,
        logo_url,
        signup_enabled,
        local_auth_enabled,
        github_auth_enabled,
        gitlab_auth_enabled,
    }))
}
