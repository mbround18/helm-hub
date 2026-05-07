use axum::{
    extract::{Request, State},
    http::header::AUTHORIZATION,
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use diesel::prelude::*;

use crate::{
    AppState,
    auth::jwt::Claims,
    auth::{hash_api_token, jwt::decode_jwt},
    db::models::{ApiToken, TouchApiToken, User},
    error::AppError,
    schema::{api_tokens, users},
};

/// Validates a `Bearer` token from the `Authorization` header.
///
/// Accepts two token types:
/// - Tokens that begin with `hhub_` are personal access tokens looked up by
///   their SHA-256 hash in the `api_tokens` table.
/// - All other tokens are treated as JWTs and validated with the application
///   secret.
pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let raw = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            AppError::Unauthorized("Missing or malformed Authorization header".into())
        })?;

    let claims = if raw.starts_with("hhub_") {
        resolve_api_token(&state, raw)?
    } else {
        decode_jwt(raw, &state.config.jwt_secret)?
    };

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

/// Resolves a personal access token to `Claims`.
///
/// Hash → DB lookup → expiry check → user load → async last_used_at update.
fn resolve_api_token(state: &AppState, raw_token: &str) -> Result<Claims, AppError> {
    let hash = hash_api_token(raw_token);
    let mut conn = state
        .db
        .get()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Lookup by hash — use the same error message for missing and expired so
    // we don't leak whether a token exists.
    let token: ApiToken = api_tokens::table
        .filter(api_tokens::token_hash.eq(&hash))
        .select(ApiToken::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::Unauthorized("Invalid or expired API token".into()))?;

    let expires_at = chrono::DateTime::parse_from_rfc3339(&token.expires_at)
        .map_err(|_| AppError::Internal("Malformed token expiry in database".into()))?;

    if Utc::now() > expires_at {
        return Err(AppError::Unauthorized(
            "Invalid or expired API token".into(),
        ));
    }

    let user: User = users::table
        .filter(users::id.eq(&token.user_id))
        .select(User::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::Unauthorized("Invalid or expired API token".into()))?;

    // Fire-and-forget: update last_used_at without blocking the request.
    let pool = state.db.clone();
    let token_id = token.id.clone();
    tokio::spawn(async move {
        if let Ok(mut c) = pool.get() {
            let _ = diesel::update(api_tokens::table.filter(api_tokens::id.eq(&token_id)))
                .set(TouchApiToken {
                    last_used_at: Some(Utc::now().to_rfc3339()),
                })
                .execute(&mut c);
        }
    });

    if user.is_banned() {
        return Err(AppError::Forbidden("Account suspended".into()));
    }

    let is_admin = user.is_admin();
    Ok(Claims {
        sub: user.id,
        username: user.username,
        is_admin,
        iat: Utc::now().timestamp(),
        exp: expires_at.timestamp(),
    })
}
