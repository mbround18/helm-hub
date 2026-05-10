use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    AppState,
    auth::{generate_api_token, hash_api_token, jwt::Claims},
    db::models::{ApiToken, NewApiToken},
    error::AppError,
    schema::api_tokens,
};

const ALLOWED_TTL_DAYS: [i64; 5] = [30, 60, 90, 180, 365];

// ── Create ────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateTokenRequest {
    pub description: String,
    /// Must be one of 30 | 60 | 90 | 180 | 365.
    pub ttl_days: i64,
}

/// `POST /api/tokens`
///
/// Creates a new personal access token for the authenticated user.
/// The raw token is returned **only in this response** — it is never stored
/// and cannot be retrieved again.
#[tracing::instrument(skip(state), fields(user = %claims.username))]
pub async fn create_token(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateTokenRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    if req.description.trim().is_empty() {
        return Err(AppError::BadRequest("description cannot be empty".into()));
    }
    if !ALLOWED_TTL_DAYS.contains(&req.ttl_days) {
        return Err(AppError::BadRequest(format!(
            "ttl_days must be one of: {ALLOWED_TTL_DAYS:?}"
        )));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let raw_token = generate_api_token();
    let token_hash = hash_api_token(&raw_token);
    let expires_at = Utc::now() + Duration::days(req.ttl_days);

    let record = NewApiToken::new(
        user_id,
        req.description.trim().to_string(),
        token_hash,
        expires_at,
    );

    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
    diesel::insert_into(api_tokens::table)
        .values(&record)
        .execute(&mut conn)
        .await?;

    tracing::info!(token_id = %record.id, ttl_days = req.ttl_days, "API token created");

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id":          record.id,
            "description": record.description,
            "token":       raw_token,      // shown exactly once
            "expires_at":  expires_at,
            "created_at":  record.created_at,
        })),
    ))
}

// ── List ──────────────────────────────────────────────────────────────────────

/// `GET /api/tokens`
///
/// Returns all tokens belonging to the authenticated user.
/// `token_hash` is never included in the serialised output (`#[serde(skip)]`).
pub async fn list_tokens(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<ApiToken>>, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
    let tokens = api_tokens::table
        .filter(api_tokens::user_id.eq(&user_id))
        .select(ApiToken::as_select())
        .order(api_tokens::created_at.desc())
        .load(&mut conn)
        .await?;
    Ok(Json(tokens))
}

// ── Delete ────────────────────────────────────────────────────────────────────

/// `DELETE /api/tokens/:id`
///
/// Revokes the token.  Only the owning user can do this.
#[tracing::instrument(skip(state), fields(user = %claims.username, token_id = %id))]
pub async fn delete_token(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
    let n = diesel::delete(
        api_tokens::table
            .filter(api_tokens::id.eq(&id))
            .filter(api_tokens::user_id.eq(&user_id)),
    )
    .execute(&mut conn)
    .await?;

    if n == 0 {
        return Err(AppError::NotFound("Token not found".into()));
    }
    tracing::info!("API token revoked");
    Ok(StatusCode::NO_CONTENT)
}
