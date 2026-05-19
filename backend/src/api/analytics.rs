//! Analytics endpoints providing insight into charts and instance-wide usage.

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    auth::jwt::Claims,
    db::user_role::UserRole,
    error::AppError,
    schema::{artifacts, artifact_versions, users},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};

// ────────────────────────────────────────────────────────────────────────────
// Types
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct InstanceAnalytics {
    pub total_users: i64,
    pub total_storage_bytes: i64,
    pub total_storage_limit_bytes: i64,
    pub total_downloads: i64,
    pub total_artifacts: i64,
    pub total_artifact_versions: i64,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageAnalytics {
    pub artifact_id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub downloads: i64,
    pub versions_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloadStatistics {
    pub total: i64,
    pub by_version: Vec<VersionStats>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionStats {
    pub version: String,
    pub downloads: i64,
}

// ────────────────────────────────────────────────────────────────────────────
// SQL Result Types
// ────────────────────────────────────────────────────────────────────────────

#[derive(QueryableByName)]
struct StorageSum {
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
    sum: Option<i64>,
}

#[derive(QueryableByName)]
struct DownloadSum {
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::BigInt>)]
    sum: Option<i64>,
}

// ────────────────────────────────────────────────────────────────────────────
// Endpoints
// ────────────────────────────────────────────────────────────────────────────

/// GET /api/analytics/instance
/// 
/// Requires: Admin role or higher
/// Returns aggregated statistics for the entire instance.
pub async fn get_instance_analytics(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<InstanceAnalytics>, AppError> {
    let user_role = claims.user_role()?;

    if !user_role.is_admin_or_higher() {
        return Err(AppError::Forbidden(
            "Admin access required to view instance analytics".into(),
        ));
    }

    let mut conn = state.db.get().await
        .map_err(|e| AppError::Pool(e.to_string()))?;

    let total_users: i64 = users::table
        .count()
        .get_result(&mut conn)
        .await?;

    let total_storage_bytes: i64 = diesel::sql_query(
        "SELECT COALESCE(SUM(storage_usage_bytes), 0) as sum FROM users"
    )
    .get_result::<StorageSum>(&mut conn)
    .await?
    .sum
    .unwrap_or(0);

    let total_storage_limit_bytes: i64 = diesel::sql_query(
        "SELECT COALESCE(SUM(storage_quota_bytes), 0) as sum FROM users"
    )
    .get_result::<StorageSum>(&mut conn)
    .await?
    .sum
    .unwrap_or(0);

    let total_downloads: i64 = diesel::sql_query(
        "SELECT COALESCE(SUM(download_count), 0) as sum FROM artifacts"
    )
    .get_result::<DownloadSum>(&mut conn)
    .await?
    .sum
    .unwrap_or(0);

    let total_artifacts: i64 = artifacts::table
        .count()
        .get_result(&mut conn)
        .await?;

    let total_artifact_versions: i64 = artifact_versions::table
        .count()
        .get_result(&mut conn)
        .await?;

    Ok(Json(InstanceAnalytics {
        total_users,
        total_storage_bytes,
        total_storage_limit_bytes,
        total_downloads,
        total_artifacts,
        total_artifact_versions,
        last_updated: Utc::now(),
    }))
}

/// GET /api/analytics/package/:owner/:chart
/// 
/// Returns analytics for a specific package/artifact.
/// Requires: Authentication + authorization check
///   - Owner/Admin: can view any artifact's analytics
///   - User: can only view their own artifacts
pub async fn get_package_analytics(
    Path((owner, chart)): Path<(String, String)>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<PackageAnalytics>, AppError> {
    let mut conn = state.db.get().await
        .map_err(|e| AppError::Pool(e.to_string()))?;

    let user_role = claims.user_role()?;
    let actor_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("Invalid user ID in token".into()))?;

    let owner_user: crate::db::models::User = users::table
        .filter(users::username.eq(&owner))
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("User '{}' not found", owner)))?;

    let artifact: crate::db::models::Artifact = artifacts::table
        .filter(artifacts::owner_id.eq(&owner_user.id))
        .filter(artifacts::name.eq(&chart))
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(
            format!("Artifact '{}/{}' not found", owner, chart)
        ))?;

    match user_role {
        UserRole::Owner | UserRole::Admin => {
            // Admin and Owner can see any analytics
        }
        UserRole::User => {
            // Regular users can only see their own artifacts
            if actor_id != owner_user.id {
                return Err(AppError::Forbidden(
                    "Cannot view another user's analytics".into(),
                ));
            }
        }
    }

    let versions_count: i64 = artifact_versions::table
        .filter(artifact_versions::artifact_id.eq(&artifact.id))
        .count()
        .get_result(&mut conn)
        .await?;

    Ok(Json(PackageAnalytics {
        artifact_id: artifact.id,
        owner_id: artifact.owner_id,
        name: artifact.name,
        downloads: artifact.download_count as i64,
        versions_count,
        created_at: artifact.created_at,
        updated_at: artifact.updated_at,
    }))
}
