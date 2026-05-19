//! Admin-only endpoints — all require the `require_auth` + `require_admin` middleware chain.
//!
//! Every state-changing action writes a row to `audit_logs` so there is
//! an immutable, timestamped record of who did what.

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    auth::jwt::Claims,
    db::{
        RlsConn,
        models::{AdminUpdateUser, Artifact, ArtifactVersion, User},
        user_role::UserRole,
    },
    error::AppError,
    schema::{artifact_versions, artifacts, users},
    services::{audit::audit, auth_providers, settings},
    utils::permissions::can_promote_to,
};

// ── GET /api/admin/users ──────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub banned_at: Option<DateTime<Utc>>,
    pub storage_usage_bytes: i64,
    pub storage_quota_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserSummary {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            is_admin: u.is_admin,
            banned_at: u.banned_at,
            storage_usage_bytes: u.storage_usage_bytes,
            storage_quota_bytes: u.storage_quota_bytes,
            created_at: u.created_at,
        }
    }
}

pub async fn list_users(
    State(_state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
) -> Result<Json<Vec<UserSummary>>, AppError> {
    let all: Vec<User> = users::table
        .select(User::as_select())
        .order(users::created_at.asc())
        .load(&mut conn)
        .await?;
    Ok(Json(all.into_iter().map(UserSummary::from).collect()))
}

// ── POST /api/admin/users/:id/promote ────────────────────────────────────────

pub async fn promote_user(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let _state = &state.0;
    let admin_id = Uuid::parse_str(&claims.sub).ok();

    let updated = diesel::update(users::table.filter(users::id.eq(&id)))
        .set(AdminUpdateUser {
            is_admin: Some(true),
            banned_at: None,
            storage_quota_bytes: None,
            role: None,
            updated_at: Utc::now(),
        })
        .execute(&mut conn)
        .await?;

    if updated == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    audit(&mut conn, admin_id, "promote", "user", Some(id), None).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── PUT /api/admin/users/:id/role ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateRoleRequest {
    /// The new role to assign (e.g., "User", "Admin", "Owner")
    pub role: String,
}

/// Update a user's role with RBAC authorization checks.
/// 
/// # Rules
/// - Owner can promote to any role
/// - Admin can promote to User or Admin (but not Owner)
/// - User cannot promote anyone
/// - Cannot downgrade an existing Owner
/// - Cannot promote a banned user
pub async fn update_user_role(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path(target_user_id): Path<Uuid>,
    Json(req): Json<UpdateRoleRequest>,
) -> Result<Json<UserSummary>, AppError> {
    let _state = &state.0;
    let actor_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;
    
    // Prevent self-modification
    if actor_id == target_user_id {
        return Err(AppError::BadRequest(
            "Cannot change your own role".into(),
        ));
    }

    // Parse the target role
    let target_role = UserRole::parse(&req.role)
        .ok_or_else(|| AppError::BadRequest(format!("Invalid role: {}", req.role)))?;

    // Get the current user (actor) role
    let actor_user: User = users::table
        .filter(users::id.eq(&actor_id))
        .select(User::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound("Actor user not found".into()))?;

    let actor_role = actor_user.role;

    // Check if actor can promote to the target role
    if !can_promote_to(actor_role, target_role) {
        return Err(AppError::Forbidden(
            format!(
                "Your role ({}) cannot promote to {}",
                actor_role.as_str(),
                target_role.as_str()
            ),
        ));
    }

    // Get the target user
    let target_user: User = users::table
        .filter(users::id.eq(&target_user_id))
        .select(User::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound("Target user not found".into()))?;

    // Prevent downgrading Owner role
    if target_user.role.is_owner() && !target_role.is_owner() {
        return Err(AppError::Forbidden(
            "Cannot downgrade an Owner role".into(),
        ));
    }

    // Prevent promoting banned users
    if target_user.is_banned() {
        return Err(AppError::BadRequest(
            "Cannot change the role of a banned user".into(),
        ));
    }

    // Record old role for audit log
    let old_role = target_user.role;

    // Update the role in the database
    let updated = diesel::update(users::table.filter(users::id.eq(&target_user_id)))
        .set(AdminUpdateUser {
            is_admin: None,
            banned_at: None,
            storage_quota_bytes: None,
            role: Some(target_role),
            updated_at: Utc::now(),
        })
        .execute(&mut conn)
        .await?;

    if updated == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    // Audit log the role change
    audit(
        &mut conn,
        Some(actor_id),
        "role_updated",
        "user",
        Some(target_user_id),
        Some(serde_json::json!({
            "old_role": old_role.as_str(),
            "new_role": target_role.as_str(),
        })),
    )
    .await?;

    // Fetch updated user and return as summary
    let updated_user: User = users::table
        .filter(users::id.eq(&target_user_id))
        .select(User::as_select())
        .first(&mut conn)
        .await?;

    Ok(Json(UserSummary::from(updated_user)))
}


// ── POST /api/admin/users/:id/ban ────────────────────────────────────────────

pub async fn ban_user(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let admin_id = Uuid::parse_str(&claims.sub).map_err(|e| AppError::Internal(e.to_string()))?;
    if id == admin_id {
        return Err(AppError::BadRequest("Cannot ban yourself".into()));
    }

    let _state = &state.0;
    let now = Utc::now();

    let updated = diesel::update(users::table.filter(users::id.eq(&id)))
        .set(AdminUpdateUser {
            is_admin: None,
            banned_at: Some(Some(now)),
            storage_quota_bytes: None,
            role: None,
            updated_at: now,
        })
        .execute(&mut conn)
        .await?;

    if updated == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    audit(&mut conn, Some(admin_id), "ban", "user", Some(id), None).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── DELETE /api/admin/users/:id ───────────────────────────────────────────────

pub async fn purge_user(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let admin_id = Uuid::parse_str(&claims.sub).map_err(|e| AppError::Internal(e.to_string()))?;
    if id == admin_id {
        return Err(AppError::BadRequest("Cannot purge yourself".into()));
    }

    let _state = &state.0;

    let user: User = users::table
        .filter(users::id.eq(&id))
        .select(User::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound("User not found".into()))?;

    // Delete artifact files from disk first (best-effort).
    let storage_root = std::path::Path::new(&state.config.charts_storage_path).join(&user.username);
    if storage_root.exists() {
        let _ = tokio::fs::remove_dir_all(&storage_root).await;
    }

    // Cascade: artifact_versions, artifacts, api_tokens, github_* are all FK'd to user.
    // PostgreSQL handles cascade if configured, but we can also do it explicitly if needed.
    // Assuming migrations set up ON DELETE CASCADE.

    diesel::delete(users::table.filter(users::id.eq(&id)))
        .execute(&mut conn)
        .await?;

    audit(
        &mut conn,
        Some(admin_id),
        "purge",
        "user",
        Some(id),
        Some(serde_json::json!({ "username": user.username })),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

// ── DELETE /api/admin/artifacts/:owner/:chart_name ───────────────────────────────

pub async fn delete_any_artifact(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path((owner, chart_name)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let _state = &state.0;
    let admin_id = Uuid::parse_str(&claims.sub).ok();

    // Resolve owner username → user_id.
    let owner_id: Uuid = users::table
        .filter(users::username.eq(&owner))
        .select(users::id)
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("User '{owner}' not found")))?;

    let artifact: Artifact = artifacts::table
        .filter(artifacts::owner_id.eq(&owner_id))
        .filter(artifacts::name.eq(&chart_name))
        .select(Artifact::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("Artifact '{chart_name}' not found")))?;

    // Collect storage paths before deleting DB rows.
    let versions: Vec<ArtifactVersion> = artifact_versions::table
        .filter(artifact_versions::artifact_id.eq(&artifact.id))
        .select(ArtifactVersion::as_select())
        .load(&mut conn)
        .await?;

    let mut total_bytes: i64 = 0;
    for v in &versions {
        let p = std::path::Path::new(&state.config.charts_storage_path).join(&v.storage_path);
        if let Ok(m) = tokio::fs::metadata(&p).await {
            total_bytes += m.len() as i64;
        }
    }

    diesel::delete(artifacts::table.filter(artifacts::id.eq(&artifact.id)))
        .execute(&mut conn)
        .await?;

    // Delete files from disk (best-effort).
    for v in &versions {
        let path = std::path::Path::new(&state.config.charts_storage_path).join(&v.storage_path);
        let _ = tokio::fs::remove_file(&path).await;
    }

    // Release storage quota for owner.
    if total_bytes > 0 {
        crate::services::quota::release_quota(&state, owner_id, total_bytes).await;
    }

    audit(
        &mut conn,
        admin_id,
        "delete_artifact",
        "artifact",
        Some(artifact.id),
        Some(serde_json::json!({ "owner": owner, "artifact": chart_name })),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

// ── PUT /api/admin/users/:id/quota ───────────────────────────────────────────

#[derive(Deserialize)]
pub struct SetQuotaBody {
    /// New quota in bytes. `null` resets to the global default.
    pub quota_bytes: Option<i64>,
}

pub async fn set_user_quota(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path(id): Path<Uuid>,
    Json(body): Json<SetQuotaBody>,
) -> Result<StatusCode, AppError> {
    let _state = &state.0;
    let admin_id = Uuid::parse_str(&claims.sub).ok();

    let updated = diesel::update(users::table.filter(users::id.eq(&id)))
        .set(AdminUpdateUser {
            is_admin: None,
            banned_at: None,
            storage_quota_bytes: Some(body.quota_bytes),
            role: None,
            updated_at: Utc::now(),
        })
        .execute(&mut conn)
        .await?;

    if updated == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    audit(
        &mut conn,
        admin_id,
        "set_quota",
        "user",
        Some(id),
        Some(serde_json::json!({ "quota_bytes": body.quota_bytes })),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

// ── PUT /api/admin/settings ───────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateSettingsBody {
    pub app_name: Option<String>,
    pub logo_url: Option<String>,
    pub signup_enabled: Option<bool>,
}

pub async fn update_settings(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Json(body): Json<UpdateSettingsBody>,
) -> Result<StatusCode, AppError> {
    let _state = &state.0;
    let admin_id = Uuid::parse_str(&claims.sub).ok();

    if let Some(ref name) = body.app_name {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.len() > 64 {
            return Err(AppError::BadRequest(
                "app_name must be 1–64 characters".into(),
            ));
        }
        settings::set(&mut conn, "app_name", trimmed).await?;
    }

    if let Some(ref url) = body.logo_url {
        let trimmed = url.trim();
        if !trimmed.is_empty()
            && !trimmed.starts_with("https://")
            && !trimmed.starts_with("http://")
        {
            return Err(AppError::BadRequest(
                "logo_url must be an http/https URL or empty".into(),
            ));
        }
        settings::set(&mut conn, "logo_url", trimmed).await?;
    }

    if let Some(enabled) = body.signup_enabled {
        settings::set(
            &mut conn,
            "signup_enabled",
            if enabled { "true" } else { "false" },
        )
        .await?;
    }

    let meta = serde_json::json!({
        "app_name": body.app_name,
        "logo_url": body.logo_url,
        "signup_enabled": body.signup_enabled,
    });
    audit(
        &mut conn,
        admin_id,
        "update_settings",
        "settings",
        None,
        Some(meta),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Observability Settings ────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct ObservabilitySettings {
    pub otel_endpoint: Option<String>,
    pub grafana_url: Option<String>,
    pub grafana_configured: bool,
}

pub async fn get_observability_settings(
    _state: State<AppState>,
    Extension(_claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
) -> Result<Json<ObservabilitySettings>, AppError> {
    let otel_endpoint = auth_providers::opt_setting(&mut conn, auth_providers::KEY_OTEL_ENDPOINT)
        .await
        .filter(|e| !e.is_empty());
    
    let grafana_url = auth_providers::opt_setting(&mut conn, auth_providers::KEY_GRAFANA_URL)
        .await
        .filter(|u| !u.is_empty());
    
    let grafana_configured = grafana_url.is_some()
        && auth_providers::opt_setting(&mut conn, auth_providers::KEY_GRAFANA_API_TOKEN)
            .await
            .is_some();

    Ok(Json(ObservabilitySettings {
        otel_endpoint,
        grafana_url,
        grafana_configured,
    }))
}

#[derive(serde::Deserialize)]
pub struct UpdateObservabilitySettingsBody {
    pub otel_endpoint: Option<String>,
    pub grafana_url: Option<String>,
    pub grafana_api_token: Option<String>,
}

pub async fn update_observability_settings(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Json(body): Json<UpdateObservabilitySettingsBody>,
) -> Result<StatusCode, AppError> {
    let admin_id = Uuid::parse_str(&claims.sub).ok();
    let encryption_key = &state.0.config.token_encryption_key;

    if let Some(ref endpoint) = body.otel_endpoint {
        let trimmed = endpoint.trim();
        if !trimmed.is_empty() {
            // Validate that it's a reasonable URL
            if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
                return Err(AppError::BadRequest(
                    "otel_endpoint must be an http/https URL or empty".into(),
                ));
            }
        }
        settings::set(&mut conn, auth_providers::KEY_OTEL_ENDPOINT, trimmed).await?;
    }

    if let Some(ref url) = body.grafana_url {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            // Validate URL
            if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
                return Err(AppError::BadRequest(
                    "grafana_url must be an http/https URL or empty".into(),
                ));
            }
            // Test connection if token is also provided
            if let Some(ref token) = body.grafana_api_token {
                let grafana_config = crate::services::observability::GrafanaConfig {
                    url: trimmed.to_string(),
                    api_token: token.trim().to_string(),
                };
                crate::services::observability::validate_grafana(&state.0.http_client, &grafana_config)
                    .await?;
            }
        }
        settings::set(&mut conn, auth_providers::KEY_GRAFANA_URL, trimmed).await?;
    }

    if let Some(ref token) = body.grafana_api_token {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            // Encrypt and store
            let encrypted = crate::services::token_crypto::encrypt_token(
                trimmed,
                encryption_key,
            )?;
            settings::set(&mut conn, auth_providers::KEY_GRAFANA_API_TOKEN, &encrypted).await?;
        } else {
            // Clear the token
            settings::set(&mut conn, auth_providers::KEY_GRAFANA_API_TOKEN, "").await?;
        }
    }

    let meta = serde_json::json!({
        "otel_endpoint": body.otel_endpoint,
        "grafana_url": body.grafana_url,
        "grafana_token_set": body.grafana_api_token.is_some(),
    });
    audit(
        &mut conn,
        admin_id,
        "update_observability_settings",
        "settings",
        None,
        Some(meta),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
