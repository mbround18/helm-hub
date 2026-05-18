use axum::{Json, extract::State};
use serde::Serialize;

use crate::{AppState, error::AppError};

#[derive(Serialize)]
pub struct AppSettingsResponse {
    pub app_name: String,
    pub logo_url: String,
    pub signup_enabled: bool,
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
    Ok(Json(AppSettingsResponse {
        app_name,
        logo_url,
        signup_enabled,
    }))
}
