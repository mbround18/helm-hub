//! GitLab OAuth App repository linking + sync handlers.
//!
//! Credentials are configured in the admin panel and stored in app_settings.
//! See auth_providers.rs for the configuration keys.
//!
//! Security notes:
//!   - `return_to` is validated to be a same-origin relative path before being
//!     embedded in the OAuth state JWT, preventing open redirect attacks.
//!   - OAuth state JWTs are signed with a **separate** secret (OAUTH_STATE_SECRET)
//!     distinct from session JWTs (JWT_SECRET).
//!   - GitLab access tokens are encrypted at rest using AES-256-GCM before being
//!     written to the database.
//!   - The GitLab OAuth scope is `read_repository` (minimal).
//!   - Internal error details are never forwarded to the browser in redirects.

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    auth::jwt::Claims,
    db::models::{GitlabConnection, GitlabRepo, NewGitlabConnection, NewGitlabRepo},
    error::AppError,
    schema::{gitlab_connections, gitlab_repos},
    services::{audit::audit, auth_providers, gitlab_sync, token_crypto},
};

// ── OAuth state JWT ───────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct OAuthState {
    user_id: Uuid,
    return_to: String,
    exp: i64,
}

fn encode_oauth_state(user_id: Uuid, return_to: &str, secret: &str) -> Result<String, AppError> {
    let claims = OAuthState {
        user_id,
        return_to: return_to.to_string(),
        exp: Utc::now().timestamp() + 600,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("OAuth state encoding failed: {e}")))
}

fn decode_oauth_state(state: &str, secret: &str) -> Result<OAuthState, AppError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    decode::<OAuthState>(
        state,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|d| d.claims)
    .map_err(|_| AppError::Unauthorized("Invalid or expired OAuth state".into()))
}

// ── Redirect safety ───────────────────────────────────────────────────────────

fn safe_return_to(raw: Option<&str>) -> &str {
    match raw {
        Some(s) if s.starts_with('/') && !s.starts_with("//") => s,
        _ => "/profile",
    }
}

// ── GitLab API helpers ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GitlabTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GitlabUser {
    id: i64,
    username: String,
    avatar_url: Option<String>,
}

async fn exchange_code(
    client: &reqwest::Client,
    base_url: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> Result<String, AppError> {
    #[derive(Serialize)]
    struct Body<'a> {
        client_id: &'a str,
        client_secret: &'a str,
        code: &'a str,
        redirect_uri: &'a str,
        grant_type: &'a str,
    }

    let token_url = format!("{}/oauth/token", base_url.trim_end_matches('/'));
    let resp = client
        .post(&token_url)
        .json(&Body {
            client_id,
            client_secret,
            code,
            redirect_uri,
            grant_type: "authorization_code",
        })
        .send()
        .await
        .map_err(|_| AppError::Internal("GitLab token exchange failed".into()))?;

    let token_resp: GitlabTokenResponse = resp
        .json()
        .await
        .map_err(|_| AppError::Internal("Failed to parse GitLab token response".into()))?;

    Ok(token_resp.access_token)
}

async fn get_gitlab_user(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
) -> Result<GitlabUser, AppError> {
    let user_url = format!("{}/api/v4/user", base_url.trim_end_matches('/'));
    let resp = client
        .get(&user_url)
        .header("PRIVATE-TOKEN", access_token)
        .header("User-Agent", "helm-hub/1.0")
        .send()
        .await
        .map_err(|_| AppError::Internal("GitLab user fetch failed".into()))?;

    resp.json::<GitlabUser>()
        .await
        .map_err(|_| AppError::Internal("Failed to parse GitLab user response".into()))
}

// ── GET /api/auth/gitlab/url ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct OAuthUrlQuery {
    pub return_to: Option<String>,
}

pub async fn oauth_url(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(q): Query<OAuthUrlQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Get GitLab credentials from admin-configured settings
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("DB connection error: {e}")))?;

    let gitlab_enabled =
        auth_providers::bool_setting(&mut conn, auth_providers::KEY_GITLAB_ENABLED, false).await;
    if !gitlab_enabled {
        return Err(AppError::Internal(
            "GitLab OAuth is not configured on this server.".into(),
        ));
    }

    let base_url = auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_BASE_URL)
        .await
        .ok_or_else(|| AppError::Internal("GitLab base URL not configured".into()))?;

    let client_id = auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_CLIENT_ID)
        .await
        .ok_or_else(|| AppError::Internal("GitLab client ID not configured".into()))?;

    let redirect_uri =
        auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_REDIRECT_URI)
            .await
            .ok_or_else(|| AppError::Internal("GitLab redirect URI not configured".into()))?;

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    // Validate return_to before embedding in the signed state.
    let return_to = safe_return_to(q.return_to.as_deref());

    let oauth_state = encode_oauth_state(user_id, return_to, &state.config.oauth_state_secret)?;

    let authorize_url = format!("{}/oauth/authorize", base_url.trim_end_matches('/'));
    let url = format!(
        "{}\
         ?client_id={client_id}\
         &redirect_uri={redirect_uri}\
         &scope=read_repository\
         &response_type=code\
         &state={oauth_state}",
        authorize_url
    );

    Ok(Json(serde_json::json!({ "url": url })))
}

// ── GET /api/auth/gitlab/callback ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub async fn oauth_callback(
    State(state): State<AppState>,
    Query(q): Query<CallbackQuery>,
) -> impl IntoResponse {
    // Generic error page — never forward internal details to the browser.
    let redirect_err = |return_to: &str| {
        Redirect::to(&format!("{}?gitlab=error", safe_return_to(Some(return_to))))
    };

    if q.error.is_some() {
        return redirect_err("/profile");
    }

    let (code, raw_state) = match (q.code, q.state) {
        (Some(c), Some(s)) => (c, s),
        _ => return redirect_err("/profile"),
    };

    let oauth_state = match decode_oauth_state(&raw_state, &state.config.oauth_state_secret) {
        Ok(s) => s,
        Err(_) => return redirect_err("/profile"),
    };

    let return_to = safe_return_to(Some(&oauth_state.return_to));

    // Get GitLab credentials from admin-configured settings
    let mut conn = match state.db.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "DB connection error during GitLab oauth_callback");
            return redirect_err(return_to);
        }
    };

    let gitlab_enabled =
        auth_providers::bool_setting(&mut conn, auth_providers::KEY_GITLAB_ENABLED, false).await;
    if !gitlab_enabled {
        return redirect_err(return_to);
    }

    let base_url =
        match auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_BASE_URL).await {
            Some(url) => url,
            None => return redirect_err(return_to),
        };

    let client_id =
        match auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_CLIENT_ID).await {
            Some(id) => id,
            None => return redirect_err(return_to),
        };

    let client_secret = match auth_providers::get_secret(
        &mut conn,
        auth_providers::KEY_GITLAB_CLIENT_SECRET,
        &state.config.token_encryption_key,
    )
    .await
    {
        Ok(Some(secret)) => secret,
        _ => return redirect_err(return_to),
    };

    let redirect_uri =
        match auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_REDIRECT_URI).await
        {
            Some(uri) => uri,
            None => return redirect_err(return_to),
        };

    let access_token = match exchange_code(
        &state.http_client,
        &base_url,
        &client_id,
        &client_secret,
        &redirect_uri,
        &code,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "GitLab token exchange error");
            return redirect_err(return_to);
        }
    };

    let gl_user = match get_gitlab_user(&state.http_client, &base_url, &access_token).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = %e, "GitLab user fetch error");
            return redirect_err(return_to);
        }
    };

    // Encrypt the access token before persisting.
    let encrypted_token =
        match token_crypto::encrypt_token(&access_token, &state.config.token_encryption_key) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(error = %e, "Token encryption error");
                return redirect_err(return_to);
            }
        };

    let gitlab_id = gl_user.id.to_string();
    let now = Utc::now();

    let existing = gitlab_connections::table
        .filter(gitlab_connections::user_id.eq(&oauth_state.user_id))
        .filter(gitlab_connections::gitlab_id.eq(&gitlab_id))
        .select(GitlabConnection::as_select())
        .first(&mut conn)
        .await
        .optional();

    match existing {
        Ok(Some(_)) => {
            if let Err(e) = diesel::update(
                gitlab_connections::table
                    .filter(gitlab_connections::user_id.eq(&oauth_state.user_id))
                    .filter(gitlab_connections::gitlab_id.eq(&gitlab_id)),
            )
            .set((
                gitlab_connections::gitlab_access_token.eq(&encrypted_token),
                gitlab_connections::gitlab_username.eq(gl_user.username.clone()),
                gitlab_connections::avatar_url.eq(gl_user.avatar_url.clone()),
                gitlab_connections::updated_at.eq(&now),
            ))
            .execute(&mut conn)
            .await
            {
                tracing::error!(error = %e, "DB update error during GitLab oauth_callback");
                return redirect_err(return_to);
            }
        }
        Ok(None) => {
            let new_conn = NewGitlabConnection::new(
                oauth_state.user_id,
                gitlab_id,
                gl_user.username.clone(),
                encrypted_token,
                gl_user.avatar_url.clone(),
            );
            if let Err(e) = diesel::insert_into(gitlab_connections::table)
                .values(&new_conn)
                .execute(&mut conn)
                .await
            {
                tracing::error!(error = %e, "DB insert error during GitLab oauth_callback");
                return redirect_err(return_to);
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "DB query error during GitLab oauth_callback");
            return redirect_err(return_to);
        }
    }

    let _ = audit(
        &mut conn,
        Some(oauth_state.user_id),
        "gitlab_connection_created",
        "gitlab_connection",
        None,
        None,
    )
    .await;

    Redirect::to(&format!("{}?gitlab=connected", return_to))
}

// ── GET /api/gitlab/connection ────────────────────────────────────────────────

pub async fn get_connection(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("DB connection failed: {e}")))?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id: {e}")))?;

    let connection = gitlab_connections::table
        .filter(gitlab_connections::user_id.eq(user_id))
        .select(GitlabConnection::as_select())
        .first(&mut conn)
        .await
        .optional()
        .map_err(|e| AppError::Internal(format!("DB error: {e}")))?;

    Ok(Json(serde_json::json!({
        "connection": connection
    })))
}

// ── DELETE /api/gitlab/connection ─────────────────────────────────────────────

pub async fn delete_connection(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("DB connection failed: {e}")))?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id: {e}")))?;

    diesel::delete(gitlab_connections::table.filter(gitlab_connections::user_id.eq(user_id)))
        .execute(&mut conn)
        .await
        .map_err(|e| AppError::Internal(format!("DB error: {e}")))?;

    audit(
        &mut conn,
        Some(user_id),
        "gitlab_connection_deleted",
        "gitlab_connection",
        None,
        None,
    )
    .await
    .ok();

    Ok(StatusCode::NO_CONTENT)
}

// ── GET /api/gitlab/repos ─────────────────────────────────────────────────────

pub async fn list_repos(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<GitlabRepo>>, AppError> {
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("DB connection failed: {e}")))?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id: {e}")))?;

    let repos = gitlab_repos::table
        .filter(gitlab_repos::user_id.eq(user_id))
        .select(GitlabRepo::as_select())
        .load(&mut conn)
        .await
        .map_err(|e| AppError::Internal(format!("DB error: {e}")))?;

    Ok(Json(repos))
}

// ── POST /api/gitlab/repos ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AddRepoRequest {
    pub slug: String,
}

pub async fn add_repo(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<AddRepoRequest>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("DB connection failed: {e}")))?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id: {e}")))?;

    let connection = gitlab_connections::table
        .filter(gitlab_connections::user_id.eq(user_id))
        .select(GitlabConnection::as_select())
        .first(&mut conn)
        .await
        .optional()
        .map_err(|e| AppError::Internal(format!("DB error: {e}")))?
        .ok_or_else(|| AppError::Unauthorized("GitLab account not connected".into()))?;

    let parts: Vec<&str> = req.slug.split('/').collect();
    if parts.len() != 2 {
        return Err(AppError::BadRequest(
            "Invalid repository slug format".into(),
        ));
    }

    let repo_owner = parts[0].to_string();
    let repo_name = parts[1].to_string();

    let new_repo = NewGitlabRepo::new(
        user_id,
        connection.id,
        repo_owner.clone(),
        repo_name.clone(),
    );

    diesel::insert_into(gitlab_repos::table)
        .values(&new_repo)
        .execute(&mut conn)
        .await
        .map_err(|_| AppError::BadRequest("Repository already linked".into()))?;

    audit(
        &mut conn,
        Some(user_id),
        "gitlab_repo_added",
        "gitlab_repo",
        None,
        Some(serde_json::json!({"repo": format!("{}/{}", repo_owner, repo_name)})),
    )
    .await
    .ok();

    Ok(StatusCode::CREATED)
}

// ── POST /api/gitlab/repos/:id/sync ───────────────────────────────────────────

pub async fn sync_repo(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(repo_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("DB connection failed: {e}")))?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id: {e}")))?;

    let repo_uuid = Uuid::parse_str(&repo_id)
        .map_err(|_| AppError::BadRequest("Invalid repository ID".into()))?;

    let repo = gitlab_repos::table
        .filter(gitlab_repos::id.eq(repo_uuid))
        .filter(gitlab_repos::user_id.eq(user_id))
        .select(GitlabRepo::as_select())
        .first(&mut conn)
        .await
        .optional()
        .map_err(|e| AppError::Internal(format!("DB error: {e}")))?
        .ok_or_else(|| AppError::NotFound("Repository not found".into()))?;

    let connection = gitlab_connections::table
        .filter(gitlab_connections::id.eq(repo.gitlab_connection_id))
        .select(GitlabConnection::as_select())
        .first(&mut conn)
        .await
        .optional()
        .map_err(|e| AppError::Internal(format!("DB error: {e}")))?
        .ok_or_else(|| AppError::Internal("Connection not found".into()))?;

    let entries = gitlab_sync::sync(
        &state,
        &mut conn,
        &connection,
        &repo.repo_owner,
        &repo.repo_name,
    )
    .await?;

    diesel::update(gitlab_repos::table.filter(gitlab_repos::id.eq(repo.id)))
        .set(gitlab_repos::last_synced_at.eq(Utc::now()))
        .execute(&mut conn)
        .await
        .ok();

    Ok(Json(serde_json::json!({ "entries": entries })))
}

// ── DELETE /api/gitlab/repos/:id ──────────────────────────────────────────────

pub async fn remove_repo(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(repo_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("DB connection failed: {e}")))?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id: {e}")))?;

    let repo_uuid = Uuid::parse_str(&repo_id)
        .map_err(|_| AppError::BadRequest("Invalid repository ID".into()))?;

    diesel::delete(
        gitlab_repos::table
            .filter(gitlab_repos::id.eq(repo_uuid))
            .filter(gitlab_repos::user_id.eq(user_id)),
    )
    .execute(&mut conn)
    .await
    .map_err(|e| AppError::Internal(format!("DB error: {e}")))?;

    Ok(StatusCode::NO_CONTENT)
}
