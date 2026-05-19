use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::{AppState, auth::jwt::Claims, db::user_role::UserRole, error::AppError};

/// Extracts the user role from JWT Claims.
///
/// Returns the parsed UserRole if successful, or an error if the role is invalid
/// or missing from the claims.
pub fn get_user_role_from_request(claims: &Claims) -> Result<UserRole, AppError> {
    claims.user_role()
}

/// Middleware that requires Admin role or higher.
///
/// Returns 403 Forbidden if the user's role is less than Admin.
///
/// # Usage
/// ```ignore
/// .layer(middleware::from_fn_with_state(
///     state.clone(),
///     auth::authorization::require_admin_or_owner,
/// ))
/// ```
pub async fn require_admin_or_owner(
    State(_state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let claims = req
        .extensions()
        .get::<Claims>()
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?
        .clone();

    let user_role = get_user_role_from_request(&claims)?;

    if user_role.is_admin_or_higher() {
        Ok(next.run(req).await)
    } else {
        Err(AppError::Forbidden(format!(
            "Admin access required. Your role: {}",
            user_role.as_str()
        )))
    }
}

/// Middleware that requires Owner role.
///
/// Returns 403 Forbidden if the user is not an Owner.
pub async fn require_owner(
    State(_state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let claims = req
        .extensions()
        .get::<Claims>()
        .ok_or_else(|| AppError::Unauthorized("Not authenticated".into()))?
        .clone();

    let user_role = get_user_role_from_request(&claims)?;

    if user_role.is_owner() {
        Ok(next.run(req).await)
    } else {
        Err(AppError::Forbidden("Owner access required".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_hierarchy() {
        // Owner >= Admin >= User
        assert!(UserRole::Owner >= UserRole::Admin);
        assert!(UserRole::Owner >= UserRole::User);
        assert!(UserRole::Admin >= UserRole::User);
        assert!((UserRole::User < UserRole::Admin));
    }

    #[test]
    fn test_get_user_role_from_request() {
        let claims = Claims::new("user-id", "testuser", false, UserRole::Admin, 24);
        let result = get_user_role_from_request(&claims);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), UserRole::Admin);
    }

    #[test]
    fn test_invalid_role() {
        let mut claims = Claims::new("user-id", "testuser", false, UserRole::User, 24);
        claims.role = Some("InvalidRole".to_string());
        let result = get_user_role_from_request(&claims);
        assert!(result.is_err());
    }

    #[test]
    fn test_legacy_token_without_role_claim() {
        let mut claims = Claims::new("user-id", "testuser", true, UserRole::Admin, 24);
        claims.role = None;
        let result = get_user_role_from_request(&claims);
        assert_eq!(result.unwrap(), UserRole::Admin);
    }

    #[test]
    fn test_is_admin_or_higher() {
        assert!(UserRole::Owner.is_admin_or_higher());
        assert!(UserRole::Admin.is_admin_or_higher());
        assert!(!UserRole::User.is_admin_or_higher());
    }

    #[test]
    fn test_is_owner() {
        assert!(UserRole::Owner.is_owner());
        assert!(!UserRole::Admin.is_owner());
        assert!(!UserRole::User.is_owner());
    }
}
