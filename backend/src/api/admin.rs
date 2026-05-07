//! Admin-only endpoints — all require the `require_auth` + `require_admin` middleware chain.
//!
//! Every state-changing action writes a row to `admin_audit_log` so there is
//! an immutable, timestamped record of who did what.

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    auth::jwt::Claims,
    db::models::{AdminUpdateUser, NewAdminAuditLog, User},
    error::AppError,
    schema::{admin_audit_log, chart_versions, charts, users},
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn audit(
    conn: &mut crate::db::DbConn,
    admin_id: &str,
    action: &str,
    target_type: &str,
    target_id: &str,
    meta: Option<serde_json::Value>,
) -> Result<(), AppError> {
    let log = NewAdminAuditLog::new(admin_id, action, target_type, target_id, meta);
    diesel::insert_into(admin_audit_log::table)
        .values(&log)
        .execute(conn)?;
    Ok(())
}

// ── GET /api/admin/users ──────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct UserSummary {
    pub id: String,
    pub username: String,
    pub email: String,
    pub is_admin: i32,
    pub banned_at: Option<String>,
    pub storage_usage_bytes: i64,
    pub storage_quota_bytes: Option<i64>,
    pub created_at: String,
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
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Result<Json<Vec<UserSummary>>, AppError> {
    let mut conn = state.db.get()?;
    let all: Vec<User> = users::table
        .select(User::as_select())
        .order(users::created_at.asc())
        .load(&mut conn)?;
    Ok(Json(all.into_iter().map(UserSummary::from).collect()))
}

// ── POST /api/admin/users/:id/promote ────────────────────────────────────────

pub async fn promote_user(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.db.get()?;

    let updated = diesel::update(users::table.filter(users::id.eq(&id)))
        .set(AdminUpdateUser {
            is_admin: Some(1),
            banned_at: None,
            storage_quota_bytes: None,
            updated_at: Utc::now().to_rfc3339(),
        })
        .execute(&mut conn)?;

    if updated == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    audit(&mut conn, &claims.sub, "promote", "user", &id, None)?;
    Ok(StatusCode::NO_CONTENT)
}

// ── POST /api/admin/users/:id/ban ────────────────────────────────────────────

pub async fn ban_user(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if id == claims.sub {
        return Err(AppError::BadRequest("Cannot ban yourself".into()));
    }

    let mut conn = state.db.get()?;
    let now = Utc::now().to_rfc3339();

    let updated = diesel::update(users::table.filter(users::id.eq(&id)))
        .set(AdminUpdateUser {
            is_admin: None,
            banned_at: Some(Some(now.clone())),
            storage_quota_bytes: None,
            updated_at: now,
        })
        .execute(&mut conn)?;

    if updated == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    audit(&mut conn, &claims.sub, "ban", "user", &id, None)?;
    Ok(StatusCode::NO_CONTENT)
}

// ── DELETE /api/admin/users/:id ───────────────────────────────────────────────

pub async fn purge_user(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if id == claims.sub {
        return Err(AppError::BadRequest("Cannot purge yourself".into()));
    }

    let mut conn = state.db.get()?;

    let user: User = users::table
        .filter(users::id.eq(&id))
        .select(User::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::NotFound("User not found".into()))?;

    // Delete chart files from disk first (best-effort).
    let storage_root = std::path::Path::new(&state.config.charts_storage_path)
        .join(&user.username);
    if storage_root.exists() {
        let _ = tokio::fs::remove_dir_all(&storage_root).await;
    }

    // Cascade: chart_versions, charts, api_tokens, github_* are all FK'd to user.
    // SQLite cascades work only when foreign_keys=ON — enforce with explicit deletes.
    diesel::delete(
        chart_versions::table.filter(
            chart_versions::chart_id.eq_any(
                charts::table
                    .filter(charts::owner_id.eq(&id))
                    .select(charts::id),
            ),
        ),
    )
    .execute(&mut conn)?;

    diesel::delete(charts::table.filter(charts::owner_id.eq(&id))).execute(&mut conn)?;
    diesel::delete(users::table.filter(users::id.eq(&id))).execute(&mut conn)?;

    audit(
        &mut conn,
        &claims.sub,
        "purge",
        "user",
        &id,
        Some(serde_json::json!({ "username": user.username })),
    )?;

    Ok(StatusCode::NO_CONTENT)
}

// ── DELETE /api/admin/charts/:owner/:chart_name ───────────────────────────────

pub async fn delete_any_chart(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((owner, chart_name)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let mut conn = state.db.get()?;

    // Resolve owner username → user_id.
    let owner_id: String = users::table
        .filter(users::username.eq(&owner))
        .select(users::id)
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("User '{owner}' not found")))?;

    let chart: crate::db::models::Chart = charts::table
        .filter(charts::owner_id.eq(&owner_id))
        .filter(charts::name.eq(&chart_name))
        .select(crate::db::models::Chart::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("Chart '{chart_name}' not found")))?;

    // Collect storage paths before deleting DB rows.
    let versions: Vec<crate::db::models::ChartVersion> = chart_versions::table
        .filter(chart_versions::chart_id.eq(&chart.id))
        .select(crate::db::models::ChartVersion::as_select())
        .load(&mut conn)?;

    let total_bytes: i64 = versions.iter()
        .map(|v| {
            let p = std::path::Path::new(&state.config.charts_storage_path).join(&v.storage_path);
            std::fs::metadata(&p).map(|m| m.len() as i64).unwrap_or(0)
        })
        .sum();

    diesel::delete(chart_versions::table.filter(chart_versions::chart_id.eq(&chart.id)))
        .execute(&mut conn)?;
    diesel::delete(charts::table.filter(charts::id.eq(&chart.id))).execute(&mut conn)?;

    // Delete files from disk (best-effort).
    for v in &versions {
        let path = std::path::Path::new(&state.config.charts_storage_path).join(&v.storage_path);
        let _ = tokio::fs::remove_file(&path).await;
    }

    // Release storage quota for owner.
    if total_bytes > 0 {
        crate::services::quota::release_quota(&state, &owner_id, total_bytes);
    }

    audit(
        &mut conn,
        &claims.sub,
        "delete_chart",
        "chart",
        &chart.id,
        Some(serde_json::json!({ "owner": owner, "chart": chart_name })),
    )?;

    Ok(StatusCode::NO_CONTENT)
}

// ── PUT /api/admin/users/:id/quota ───────────────────────────────────────────

#[derive(Deserialize)]
pub struct SetQuotaBody {
    /// New quota in bytes. `null` resets to the global default.
    pub quota_bytes: Option<i64>,
}

pub async fn set_user_quota(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(id): Path<String>,
    Json(body): Json<SetQuotaBody>,
) -> Result<StatusCode, AppError> {
    let mut conn = state.db.get()?;

    let updated = diesel::update(users::table.filter(users::id.eq(&id)))
        .set(AdminUpdateUser {
            is_admin: None,
            banned_at: None,
            storage_quota_bytes: Some(body.quota_bytes),
            updated_at: Utc::now().to_rfc3339(),
        })
        .execute(&mut conn)?;

    if updated == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    audit(
        &mut conn,
        &claims.sub,
        "set_quota",
        "user",
        &id,
        Some(serde_json::json!({ "quota_bytes": body.quota_bytes })),
    )?;

    Ok(StatusCode::NO_CONTENT)
}
