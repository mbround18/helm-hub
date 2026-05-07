use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};

use crate::{auth::jwt::decode_jwt, error::AppError, AppState};

/// Extracts and validates the Bearer JWT from the `Authorization` header,
/// then attaches the decoded `Claims` as a request extension.
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Missing or malformed Authorization header".into()))?;

    let claims = decode_jwt(token, &state.config.jwt_secret)?;
    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

/// Same as `require_auth` but additionally enforces that the caller is an admin.
pub async fn require_admin(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".into()))?;

    let claims = decode_jwt(token, &state.config.jwt_secret)?;

    if !claims.is_admin {
        return Err(AppError::Forbidden("Admin access required".into()));
    }

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}
