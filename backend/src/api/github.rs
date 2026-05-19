//! GitHub OAuth App repository linking + sync handlers.
//!
//! Credentials are now configured in the admin panel and stored in app_settings.
//! See auth_providers.rs for the configuration keys.
//!
//! Security notes:
//!   - `return_to` is validated to be a same-origin relative path before being
//!     embedded in the OAuth state JWT, preventing open redirect attacks.
//!   - OAuth state JWTs are signed with a **separate** secret (OAUTH_STATE_SECRET)
//!     distinct from session JWTs (JWT_SECRET).
//!   - GitHub access tokens are encrypted at rest using AES-256-GCM before being
//!     written to the database.
//!   - The GitHub OAuth scope is `read:user public_repo` (minimal) rather than
//!     the overprivileged `repo` (full read/write on all repositories).
//!   - Internal error details are never forwarded to the browser in redirects.

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use chrono::Utc;
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel_async::RunQueryDsl;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    auth::jwt::Claims,
    db::{
        RlsConn,
        models::{GithubConnection, GithubRepo, NewGithubConnection, NewGithubRepo, User},
        set_current_user,
    },
    error::AppError,
    schema::{github_connections, github_repos, users},
    services::{audit::audit, auth_providers, github_sync, token_crypto},
};

#[derive(Serialize)]
pub struct RepoSyncStatus {
    pub status: String,
    pub started_at: Option<chrono::DateTime<Utc>>,
    pub finished_at: Option<chrono::DateTime<Utc>>,
    pub error: Option<String>,
    pub report: Option<github_sync::SyncReport>,
}

fn to_repo_sync_status(repo: &GithubRepo) -> RepoSyncStatus {
    let report = repo
        .last_sync_report
        .as_ref()
        .and_then(|v| serde_json::from_value::<github_sync::SyncReport>(v.clone()).ok());
    RepoSyncStatus {
        status: repo.sync_status.clone(),
        started_at: repo.sync_started_at,
        finished_at: repo.sync_finished_at,
        error: repo.sync_error.clone(),
        report,
    }
}

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

/// Accepts only same-origin relative paths to prevent open redirect attacks.
///
/// A safe `return_to` must start with `/` but must NOT start with `//`
/// (protocol-relative URL, which browsers treat as an absolute URL).
fn safe_return_to(raw: Option<&str>) -> &str {
    match raw {
        Some(s) if s.starts_with('/') && !s.starts_with("//") => s,
        _ => "/profile",
    }
}

// ── GitHub API helpers ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GithubTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GithubUser {
    id: i64,
    login: String,
    avatar_url: Option<String>,
}

async fn exchange_code(
    client: &reqwest::Client,
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
    }

    let resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "helm-hub/1.0")
        .json(&Body {
            client_id,
            client_secret,
            code,
            redirect_uri,
        })
        .send()
        .await
        .map_err(|_| AppError::Internal("GitHub token exchange failed".into()))?;

    let token_resp: GithubTokenResponse = resp
        .json()
        .await
        .map_err(|_| AppError::Internal("Failed to parse GitHub token response".into()))?;

    Ok(token_resp.access_token)
}

async fn get_github_user(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<GithubUser, AppError> {
    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "helm-hub/1.0")
        .send()
        .await
        .map_err(|_| AppError::Internal("GitHub user fetch failed".into()))?;

    resp.json::<GithubUser>()
        .await
        .map_err(|_| AppError::Internal("Failed to parse GitHub user response".into()))
}

#[allow(dead_code)]
fn github_not_configured() -> AppError {
    AppError::Internal(
        "GitHub OAuth is not configured on this server. \
         Set GITHUB_CLIENT_ID, GITHUB_CLIENT_SECRET, and GITHUB_REDIRECT_URI."
            .into(),
    )
}

// ── GET /api/auth/github/url ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct OAuthUrlQuery {
    pub return_to: Option<String>,
}

pub async fn oauth_url(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(q): Query<OAuthUrlQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Get GitHub credentials from admin-configured settings
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Internal(format!("DB connection error: {e}")))?;

    let github_enabled =
        auth_providers::bool_setting(&mut conn, auth_providers::KEY_GITHUB_ENABLED, false).await;
    if !github_enabled {
        return Err(AppError::Internal(
            "GitHub OAuth is not configured on this server.".into(),
        ));
    }

    let client_id = auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITHUB_CLIENT_ID)
        .await
        .ok_or_else(|| AppError::Internal("GitHub client ID not configured".into()))?;

    let redirect_uri =
        auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITHUB_REDIRECT_URI)
            .await
            .ok_or_else(|| AppError::Internal("GitHub redirect URI not configured".into()))?;

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    // Validate return_to before embedding in the signed state.
    let return_to = safe_return_to(q.return_to.as_deref());

    let oauth_state = encode_oauth_state(user_id, return_to, &state.config.oauth_state_secret)?;

    // Scope: `read:user` (profile) + `public_repo` (read public releases).
    // We deliberately do NOT request the `repo` scope (full private repo access).
    let url = format!(
        "https://github.com/login/oauth/authorize\
         ?client_id={client_id}\
         &redirect_uri={redirect_uri}\
         &scope=read%3Auser%20public_repo\
         &state={oauth_state}"
    );

    Ok(Json(serde_json::json!({ "url": url })))
}

// ── GET /api/auth/github/callback ─────────────────────────────────────────────

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
        Redirect::to(&format!("{}?github=error", safe_return_to(Some(return_to))))
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

    // Get GitHub credentials from admin-configured settings
    let mut conn = match state.db.get().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "DB connection error during GitHub oauth_callback");
            return redirect_err(return_to);
        }
    };

    let github_enabled =
        auth_providers::bool_setting(&mut conn, auth_providers::KEY_GITHUB_ENABLED, false).await;
    if !github_enabled {
        return redirect_err(return_to);
    }

    let client_id =
        match auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITHUB_CLIENT_ID).await {
            Some(id) => id,
            None => return redirect_err(return_to),
        };

    let client_secret = match auth_providers::get_secret(
        &mut conn,
        auth_providers::KEY_GITHUB_CLIENT_SECRET,
        &state.config.token_encryption_key,
    )
    .await
    {
        Ok(Some(secret)) => secret,
        _ => return redirect_err(return_to),
    };

    let redirect_uri =
        match auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITHUB_REDIRECT_URI).await
        {
            Some(uri) => uri,
            None => return redirect_err(return_to),
        };

    let access_token = match exchange_code(
        &state.http_client,
        &client_id,
        &client_secret,
        &redirect_uri,
        &code,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "GitHub token exchange error");
            return redirect_err(return_to);
        }
    };

    let gh_user = match get_github_user(&state.http_client, &access_token).await {
        Ok(u) => u,
        Err(e) => {
            tracing::error!(error = %e, "GitHub user fetch error");
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

    let github_id = gh_user.id.to_string();
    let now = Utc::now();

    let existing = github_connections::table
        .filter(github_connections::user_id.eq(&oauth_state.user_id))
        .filter(github_connections::github_id.eq(&github_id))
        .select(GithubConnection::as_select())
        .first(&mut conn)
        .await
        .optional();

    match existing {
        Ok(Some(_)) => {
            if let Err(e) = diesel::update(
                github_connections::table
                    .filter(github_connections::user_id.eq(&oauth_state.user_id))
                    .filter(github_connections::github_id.eq(&github_id)),
            )
            .set((
                github_connections::github_access_token.eq(&encrypted_token),
                github_connections::github_username.eq(gh_user.login.clone()),
                github_connections::avatar_url.eq(gh_user.avatar_url.clone()),
                github_connections::updated_at.eq(&now),
            ))
            .execute(&mut conn)
            .await
            {
                tracing::error!(error = %e, "DB update error during GitHub oauth_callback");
                return redirect_err(return_to);
            }
        }
        Ok(None) => {
            let new_conn = NewGithubConnection::new(
                oauth_state.user_id,
                github_id,
                gh_user.login.clone(),
                encrypted_token,
                gh_user.avatar_url.clone(),
            );
            if let Err(e) = diesel::insert_into(github_connections::table)
                .values(&new_conn)
                .execute(&mut conn)
                .await
            {
                tracing::error!(error = %e, "DB insert error during GitHub oauth_callback");
                return redirect_err(return_to);
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "DB query error during GitHub oauth_callback");
            return redirect_err(return_to);
        }
    }

    let _ = audit(
        &mut conn,
        Some(oauth_state.user_id),
        "link_github",
        "github_connection",
        None,
        Some(serde_json::json!({ "github_username": gh_user.login })),
    )
    .await;

    Redirect::to(&format!("{return_to}?github=connected"))
}

// ── GET /api/github/connection ────────────────────────────────────────────────

pub async fn get_connection(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;
    let connection = github_connections::table
        .filter(github_connections::user_id.eq(&user_id))
        .select(GithubConnection::as_select())
        .first(&mut conn)
        .await
        .optional()?;

    Ok(Json(serde_json::json!({ "connection": connection })))
}

// ── DELETE /api/github/connection ─────────────────────────────────────────────

pub async fn delete_connection(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<StatusCode, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;
    let n =
        diesel::delete(github_connections::table.filter(github_connections::user_id.eq(&user_id)))
            .execute(&mut conn)
            .await?;

    if n > 0 {
        let _ = audit(
            &mut conn,
            Some(user_id),
            "unlink_github",
            "github_connection",
            None,
            None,
        )
        .await;
    }

    Ok(StatusCode::NO_CONTENT)
}

// ── GET /api/github/repos ─────────────────────────────────────────────────────

pub async fn list_repos(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<GithubRepo>>, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;
    let repos = github_repos::table
        .filter(github_repos::user_id.eq(&user_id))
        .select(GithubRepo::as_select())
        .order(github_repos::created_at.asc())
        .load(&mut conn)
        .await?;
    Ok(Json(repos))
}

// ── POST /api/github/repos ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AddRepoBody {
    pub repo: String,
}

pub async fn add_repo(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<AddRepoBody>,
) -> Result<(StatusCode, Json<GithubRepo>), AppError> {
    let parts: Vec<&str> = body.repo.splitn(2, '/').collect();
    let repo_owner = parts.get(0).map_or("", |s| s.trim());
    let repo_name = parts.get(1).map_or("", |s| s.trim());
    if parts.len() != 2 || repo_owner.is_empty() || repo_name.is_empty() {
        return Err(AppError::BadRequest(
            "repo must be in the form `owner/repo`".into(),
        ));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;

    let gh_conn = github_connections::table
        .filter(github_connections::user_id.eq(&user_id))
        .select(GithubConnection::as_select())
        .first(&mut conn)
        .await
        .optional()?
        .ok_or_else(|| {
            AppError::BadRequest("Link your GitHub account before adding repositories.".into())
        })?;

    let new_repo = NewGithubRepo::new(
        user_id,
        gh_conn.id,
        repo_owner.to_string(),
        repo_name.to_string(),
    );

    diesel::insert_into(github_repos::table)
        .values(&new_repo)
        .on_conflict((
            github_repos::github_connection_id,
            github_repos::repo_owner,
            github_repos::repo_name,
        ))
        .do_update()
        .set((
            github_repos::user_id.eq(excluded(github_repos::user_id)),
            github_repos::github_connection_id.eq(excluded(github_repos::github_connection_id)),
        ))
        .execute(&mut conn)
        .await?;

    let inserted = github_repos::table
        .filter(github_repos::github_connection_id.eq(&gh_conn.id))
        .filter(github_repos::repo_owner.eq(repo_owner))
        .filter(github_repos::repo_name.eq(repo_name))
        .select(GithubRepo::as_select())
        .first(&mut conn)
        .await?;

    let _ = audit(
        &mut conn,
        Some(user_id),
        "add_github_repo",
        "github_repo",
        Some(inserted.id),
        Some(serde_json::json!({
            "owner": inserted.repo_owner,
            "name": inserted.repo_name,
        })),
    )
    .await;

    Ok((StatusCode::CREATED, Json(inserted)))
}

// ── DELETE /api/github/repos/:id ─────────────────────────────────────────────

pub async fn remove_repo(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;
    let deleted = diesel::delete(
        github_repos::table
            .filter(github_repos::id.eq(&id))
            .filter(github_repos::user_id.eq(&user_id)),
    )
    .execute(&mut conn)
    .await?;

    if deleted > 0 {
        let _ = audit(
            &mut conn,
            Some(user_id),
            "remove_github_repo",
            "github_repo",
            Some(id),
            None,
        )
        .await;
    }

    Ok(StatusCode::NO_CONTENT)
}

// ── POST /api/github/repos/:id/sync ──────────────────────────────────────────

pub async fn get_sync_status(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path(id): Path<Uuid>,
) -> Result<Json<RepoSyncStatus>, AppError> {
    let _state = state;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let repo = github_repos::table
        .filter(github_repos::id.eq(&id))
        .filter(github_repos::user_id.eq(&user_id))
        .select(GithubRepo::as_select())
        .first(&mut conn)
        .await
        .optional()?
        .ok_or_else(|| AppError::NotFound("Repository not found".into()))?;

    Ok(Json(to_repo_sync_status(&repo)))
}

pub async fn sync_repo(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<RepoSyncStatus>), AppError> {
    let state = state.0;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let repo = github_repos::table
        .filter(github_repos::id.eq(&id))
        .filter(github_repos::user_id.eq(&user_id))
        .select(GithubRepo::as_select())
        .first(&mut conn)
        .await
        .optional()?
        .ok_or_else(|| AppError::NotFound("Repository not found".into()))?;

    if repo.sync_status == "in_progress" {
        return Ok((StatusCode::ACCEPTED, Json(to_repo_sync_status(&repo))));
    }

    let started_at = Utc::now();
    diesel::update(github_repos::table.filter(github_repos::id.eq(&repo.id)))
        .set((
            github_repos::sync_status.eq("in_progress"),
            github_repos::sync_started_at.eq(Some(started_at)),
            github_repos::sync_finished_at.eq::<Option<chrono::DateTime<Utc>>>(None),
            github_repos::sync_error.eq::<Option<String>>(None),
            github_repos::last_sync_report.eq::<Option<serde_json::Value>>(None),
        ))
        .execute(&mut conn)
        .await?;

    let mut queued_repo = repo.clone();
    queued_repo.sync_status = "in_progress".to_string();
    queued_repo.sync_started_at = Some(started_at);
    queued_repo.sync_finished_at = None;
    queued_repo.sync_error = None;
    queued_repo.last_sync_report = None;

    let state_bg = state.clone();
    let repo_id = repo.id;
    let actor_id = user_id;
    let actor_is_admin = claims.is_admin;
    let actor_username = claims.username.clone();
    let encryption_key = state.config.token_encryption_key.clone();
    tokio::spawn(async move {
        let mut bg_conn = match state_bg.db.get().await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "Failed to get DB connection for async GitHub sync");
                return;
            }
        };

        if let Err(e) = set_current_user(&mut bg_conn, &actor_id.to_string(), actor_is_admin).await
        {
            tracing::error!(error = %e, "Failed to set RLS context for async GitHub sync");
            return;
        }

        let repo = match github_repos::table
            .filter(github_repos::id.eq(&repo_id))
            .filter(github_repos::user_id.eq(&actor_id))
            .select(GithubRepo::as_select())
            .first(&mut bg_conn)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, "Async GitHub sync repo lookup failed");
                return;
            }
        };

        let gh_conn = match github_connections::table
            .find(&repo.github_connection_id)
            .select(GithubConnection::as_select())
            .first(&mut bg_conn)
            .await
        {
            Ok(c) => c,
            Err(e) => {
                let _ = diesel::update(github_repos::table.filter(github_repos::id.eq(&repo_id)))
                    .set((
                        github_repos::sync_status.eq("failed"),
                        github_repos::sync_finished_at.eq(Some(Utc::now())),
                        github_repos::sync_error.eq(Some(format!("Connection not found: {e}"))),
                    ))
                    .execute(&mut bg_conn)
                    .await;
                return;
            }
        };

        let user = match users::table
            .find(&actor_id)
            .select(User::as_select())
            .first(&mut bg_conn)
            .await
        {
            Ok(u) => u,
            Err(e) => {
                let _ = diesel::update(github_repos::table.filter(github_repos::id.eq(&repo_id)))
                    .set((
                        github_repos::sync_status.eq("failed"),
                        github_repos::sync_finished_at.eq(Some(Utc::now())),
                        github_repos::sync_error.eq(Some(format!("User not found: {e}"))),
                    ))
                    .execute(&mut bg_conn)
                    .await;
                return;
            }
        };

        let access_token =
            match token_crypto::decrypt_token(&gh_conn.github_access_token, &encryption_key) {
                Ok(token) => token,
                Err(e) => {
                    let _ =
                        diesel::update(github_repos::table.filter(github_repos::id.eq(&repo_id)))
                            .set((
                                github_repos::sync_status.eq("failed"),
                                github_repos::sync_finished_at.eq(Some(Utc::now())),
                                github_repos::sync_error
                                    .eq(Some(format!("Token decrypt failed: {e}"))),
                            ))
                            .execute(&mut bg_conn)
                            .await;
                    return;
                }
            };

        match github_sync::sync_repo(&state_bg, &mut bg_conn, &repo, &access_token, &user).await {
            Ok(report) => {
                let finished_at = Utc::now();
                let report_json = serde_json::to_value(&report).ok();
                let _ = diesel::update(github_repos::table.filter(github_repos::id.eq(&repo_id)))
                    .set((
                        github_repos::sync_status.eq("completed"),
                        github_repos::sync_finished_at.eq(Some(finished_at)),
                        github_repos::sync_error.eq::<Option<String>>(None),
                        github_repos::last_sync_report.eq(report_json),
                    ))
                    .execute(&mut bg_conn)
                    .await;

                let _ = audit(
                    &mut bg_conn,
                    Some(actor_id),
                    "sync_github_repo",
                    "github_repo",
                    Some(repo.id),
                    Some(serde_json::json!({
                        "repo": format!("{}/{}", repo.repo_owner, repo.repo_name),
                        "imported": report.entries.iter().filter(|e| e.status == "imported").count(),
                        "failed": report.entries.iter().filter(|e| e.status == "failed").count(),
                        "triggered_by": actor_username,
                    })),
                )
                .await;
            }
            Err(e) => {
                let _ = diesel::update(github_repos::table.filter(github_repos::id.eq(&repo_id)))
                    .set((
                        github_repos::sync_status.eq("failed"),
                        github_repos::sync_finished_at.eq(Some(Utc::now())),
                        github_repos::sync_error.eq(Some(e.to_string())),
                    ))
                    .execute(&mut bg_conn)
                    .await;
            }
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(to_repo_sync_status(&queued_repo)),
    ))
}
