use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use reqwest::Method;

use crate::AppState;
use crate::error::AppError;

/// Proxies Grafana Faro telemetry from the frontend to the internal collector.
///
/// Validates that the request comes from the configured `frontend_origin`
/// and forwards the payload to the internal OTel/Faro collector.
pub async fn faro_proxy(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Result<impl IntoResponse, AppError> {
    // 1. Validate Origin
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("Missing Origin header".into()))?;

    if origin != state.config.frontend_origin {
        tracing::warn!(%origin, expected = %state.config.frontend_origin, "Faro proxy: rejected invalid origin");
        return Err(AppError::Forbidden("Invalid origin".into()));
    }

    // 2. Resolve internal collector URL
    let collector_url = std::env::var("INTERNAL_FARO_URL")
        .ok()
        .or_else(|| state.config.otel_collector_endpoint.clone())
        .ok_or_else(|| {
            AppError::Internal("No internal collector configured for Faro proxy".into())
        })?;

    // 3. Forward the request
    let body = axum::body::to_bytes(req.into_body(), 10 * 1024 * 1024) // 10MB limit
        .await
        .map_err(|e| AppError::BadRequest(format!("Failed to read body: {e}")))?;

    let mut forward_req = state
        .http_client
        .request(Method::POST, &collector_url)
        .body(body);

    // Forward relevant headers (e.g. content-type)
    if let Some(ct) = headers.get(header::CONTENT_TYPE) {
        forward_req = forward_req.header(header::CONTENT_TYPE, ct);
    }

    let response = forward_req.send().await.map_err(|e| {
        tracing::error!(error = %e, "Failed to forward Faro data to collector");
        AppError::Internal("Failed to forward telemetry".into())
    })?;

    Ok(StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
}
