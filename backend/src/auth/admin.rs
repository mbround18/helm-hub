use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::{AppState, auth::jwt::Claims, error::AppError};

/// Middleware that gates access to admin-only routes.
///
/// Runs *after* `require_auth` (which populates the `Claims` extension).
/// Returns 403 Forbidden if the authenticated user is not an admin.
pub async fn require_admin(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let claims = req
        .extensions()
        .get::<Claims>()
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?;

    // Also auto-promote the bootstrap admin at runtime (no DB write needed).
    let is_admin = claims.is_admin
        || state
            .config
            .admin_username
            .as_deref()
            .is_some_and(|name| name == claims.username);

    if !is_admin {
        return Err(AppError::Forbidden("Admin access required".into()));
    }

    Ok(next.run(req).await)
}
