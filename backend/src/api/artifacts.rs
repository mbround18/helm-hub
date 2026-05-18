use axum::{
    Extension, Json,
    body::Bytes,
    extract::{Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path as FsPath;
use uuid::Uuid;

// ── Download-path guard ───────────────────────────────────────────────────────

/// Resolves `relative` against `root`, rejecting any path that would escape
/// the root directory.  Used before every filesystem read or delete to guard
/// against a tampered `storage_path` value in the database.
fn safe_join(root: &FsPath, relative: &str) -> Result<std::path::PathBuf, crate::error::AppError> {
    use std::path::Component;
    for component in FsPath::new(relative).components() {
        if matches!(component, Component::ParentDir | Component::RootDir) {
            return Err(crate::error::AppError::Internal(
                "Dangerous component in storage_path".into(),
            ));
        }
    }
    Ok(root.join(relative))
}

// ── Upload limits (also enforced by DefaultBodyLimit in main.rs) ──────────────
const MAX_ARTIFACTS_PER_REQUEST: usize = 20;

// Metric names — defined as constants so they're greppable.
const METRIC_UPLOADS_TOTAL: &str = "helm_hub_artifact_uploads_total";
const METRIC_UPLOAD_ERRORS_TOTAL: &str = "helm_hub_artifact_upload_errors_total";
const METRIC_SCANS_TOTAL: &str = "helm_hub_clamav_scans_total";
const METRIC_DOWNLOADS_TOTAL: &str = "helm_hub_artifact_downloads_total";

#[derive(Serialize)]
pub(crate) struct UploadedArtifact {
    pub(crate) artifact: String,
    pub(crate) version: String,
    pub(crate) owner: String,
}

#[derive(Serialize)]
struct FailedArtifact {
    error: String,
}

use crate::{
    AppState,
    auth::jwt::Claims,
    db::{
        RlsConn,
        models::{Artifact, ArtifactVersion, NewArtifact, NewArtifactVersion},
    },
    error::AppError,
    schema::{artifact_versions, artifacts, users},
    services::{
        audit::audit,
        chart_extractor::{extract_chart_metadata, parse_chart_yaml, persist_chart},
        clamav::{ScanOutcome, scan_file},
        quota,
    },
};

/// `POST /api/artifacts/:owner`
///
/// Accepts one or more `chart` fields in a single multipart request.  Each
/// field is scanned, extracted, and persisted independently.  The response
/// always has HTTP 200 with `uploaded` and `failed` arrays so the caller can
/// surface per-file results without treating the whole request as an error.
///
/// curl example (single):
///   curl -X POST -H "Authorization: Bearer $TOKEN" \
///        -F "chart=@mychart-1.0.0.tgz" \
///        https://hub/api/artifacts/<owner>
///
/// curl example (multi):
///   curl -X POST -H "Authorization: Bearer $TOKEN" \
///        -F "chart=@chart1-1.0.0.tgz" -F "chart=@chart2-2.0.0.tgz" \
///        https://hub/api/artifacts/<owner>
#[tracing::instrument(skip(state, multipart, conn), fields(owner = %owner))]
pub async fn upload_artifact(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path(owner): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let state = &state.0;
    if claims.username != owner {
        return Err(AppError::Forbidden(
            "Cannot upload to another user's namespace".into(),
        ));
    }

    // ── Collect all `chart` fields ────────────────────────────────────────────
    let mut payloads: Vec<Bytes> = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        if field.name() == Some("chart") {
            payloads.push(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
        }
    }

    if payloads.is_empty() {
        return Err(AppError::BadRequest(
            "No `chart` field(s) found in multipart body".into(),
        ));
    }
    if payloads.len() > MAX_ARTIFACTS_PER_REQUEST {
        return Err(AppError::BadRequest(format!(
            "Maximum {MAX_ARTIFACTS_PER_REQUEST} artifacts per request"
        )));
    }
    for payload in &payloads {
        if payload.len() > state.config.max_chart_file_bytes {
            return Err(AppError::BadRequest(format!(
                "Artifact file exceeds the maximum size of {} MiB",
                state.config.max_chart_file_bytes / 1024 / 1024
            )));
        }
    }

    tokio::fs::create_dir_all(&state.config.temp_upload_dir).await?;

    // ── Process each artifact independently ─────────────────────────────────────
    let mut uploaded: Vec<UploadedArtifact> = Vec::new();
    let mut failed: Vec<FailedArtifact> = Vec::new();

    for bytes in payloads {
        let temp_path = FsPath::new(&state.config.temp_upload_dir)
            .join(format!("helm-upload-{}.tgz", Uuid::new_v4()));

        tokio::fs::write(&temp_path, &bytes).await?;

        let outcome =
            scan_and_persist(&state, &mut conn, &claims.sub, &owner, &bytes, &temp_path).await;

        if let Err(e) = tokio::fs::remove_file(&temp_path).await {
            tracing::warn!(path = %temp_path.display(), error = %e, "Failed to remove temp upload file");
        }

        match outcome {
            Ok(ok) => uploaded.push(ok),
            Err(e) => {
                metrics::counter!(METRIC_UPLOAD_ERRORS_TOTAL, "reason" => "processing_error")
                    .increment(1);
                failed.push(FailedArtifact {
                    error: e.to_string(),
                });
            }
        }
    }

    let status = if failed.is_empty() {
        StatusCode::OK
    } else {
        // 207 Multi-Status: some succeeded, some failed — or all failed.
        StatusCode::MULTI_STATUS
    };

    Ok((
        status,
        Json(serde_json::json!({
            "uploaded": uploaded,
            "failed": failed,
        })),
    ))
}

/// Runs the full security + persistence pipeline for a single artifact file.
///
/// The caller is responsible for writing `bytes` to `temp_path` beforehand and
/// cleaning up the temp file afterwards regardless of the return value.
/// `user_id_str` is the UUID string of the owning user; `owner` is their username (the
/// namespace artifacts are stored under).
#[tracing::instrument(
    skip(state, bytes, conn),
    fields(owner, artifact.name = tracing::field::Empty, artifact.version = tracing::field::Empty)
)]
pub(crate) async fn scan_and_persist(
    state: &crate::AppState,
    conn: &mut crate::db::DbConn,
    user_id_str: &str,
    owner: &str,
    bytes: &Bytes,
    temp_path: &std::path::Path,
) -> Result<UploadedArtifact, AppError> {
    let user_id = Uuid::parse_str(user_id_str)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    // ── ClamAV virus scan ─────────────────────────────────────────────────────
    if state.config.clamav_enabled {
        tracing::debug!(path = %temp_path.display(), "Running ClamAV scan");

        match scan_file(&state.config.clamd_socket, temp_path).await? {
            ScanOutcome::Clean => {
                tracing::info!(owner, "Artifact scan passed — proceeding with upload");
                metrics::counter!(METRIC_SCANS_TOTAL, "result" => "clean").increment(1);
            }
            ScanOutcome::Infected(virus) => {
                tracing::warn!(owner, virus_name = %virus, "Infected artifact upload blocked");
                metrics::counter!(METRIC_SCANS_TOTAL, "result" => "infected").increment(1);
                return Err(AppError::InfectedFile(format!(
                    "Upload rejected: virus/malware detected ({virus})"
                )));
            }
        }
    } else {
        tracing::warn!("CLAMAV_ENABLED=false — skipping virus scan (not for production)");
    }

    // ── Extract Chart.yaml / values.yaml ──────────────────────────────────────
    let extracted = extract_chart_metadata(bytes)
        .map_err(|e| AppError::BadRequest(format!("Invalid Helm chart archive: {e}")))?;

    let (artifact_name, version, app_version, description) =
        parse_chart_yaml(&extracted.chart_yaml).ok_or_else(|| {
            AppError::BadRequest("Chart.yaml missing required fields (name, version)".into())
        })?;

    // ── Quota check (atomic reserve before touching disk) ─────────────────────
    let artifact_bytes = bytes.len() as i64;
    {
        quota::reserve_quota(
            conn,
            user_id,
            artifact_bytes,
            state.config.default_storage_quota_bytes,
        )
        .await?;
    }

    // ── Persist to permanent storage ──────────────────────────────────────────
    let storage_root = FsPath::new(&state.config.charts_storage_path);
    let storage_path = match persist_chart(storage_root, owner, &artifact_name, &version, bytes) {
        Ok(p) => p,
        Err(e) => {
            // Release the reserved quota since the file wasn't written.
            quota::release_quota(state, user_id, artifact_bytes).await;
            return Err(AppError::Io(e));
        }
    };

    // ── Transactional DB update with Advisory Lock ────────────────────────────
    let result = conn
        .transaction::<UploadedArtifact, AppError, _>(async |conn| {
            let artifact: Artifact = match artifacts::table
                .filter(artifacts::owner_id.eq(user_id))
                .filter(artifacts::name.eq(&artifact_name))
                .select(Artifact::as_select())
                .first(conn)
                .await
                .optional()?
            {
                Some(a) => a,
                None => {
                    let new_artifact = NewArtifact::new(
                        user_id,
                        artifact_name.clone(),
                        "helm".to_string(),
                        description.clone(),
                    );
                    diesel::insert_into(artifacts::table)
                        .values(&new_artifact)
                        .execute(conn)
                        .await?;
                    artifacts::table
                        .filter(artifacts::id.eq(&new_artifact.id))
                        .select(Artifact::as_select())
                        .first(conn)
                        .await?
                }
            };

            // ── Advisory Lock ─────────────────────────────────────────────────────
            // Prevent concurrent uploads of the same version for the same artifact.
            let lock_key = format!("{}-{}", artifact.id, version);
            let lock_id = crc32fast::hash(lock_key.as_bytes()) as i64;
            diesel::sql_query("SELECT pg_advisory_xact_lock($1)")
                .bind::<diesel::sql_types::BigInt, _>(lock_id)
                .execute(conn)
                .await?;

            let exists = artifact_versions::table
                .filter(artifact_versions::artifact_id.eq(&artifact.id))
                .filter(artifact_versions::version.eq(&version))
                .count()
                .get_result::<i64>(conn)
                .await?
                > 0;

            if exists {
                return Err(AppError::Conflict(format!(
                    "Version {version} of artifact {artifact_name} already exists"
                )));
            }

            // ── Index in database ─────────────────────────────────────────────────────
            let mut metadata = serde_json::json!({
                "app_version": app_version,
                "description": description,
            });
            if let Ok(chart_yaml) = serde_json::from_str::<serde_json::Value>(&extracted.chart_yaml)
            {
                metadata["chart_yaml"] = chart_yaml;
            }
            if let Some(v) = extracted.values_yaml.as_deref()
                && let Ok(values_yaml) = serde_json::from_str::<serde_json::Value>(v)
            {
                metadata["values_yaml"] = values_yaml;
            }
            if let Some(s) = extracted.schema_json.as_deref()
                && let Ok(schema_json) = serde_json::from_str::<serde_json::Value>(s)
            {
                metadata["schema_json"] = schema_json;
            }

            let new_version = NewArtifactVersion {
                id: Uuid::new_v4(),
                artifact_id: artifact.id,
                version: version.clone(),
                digest: hex::decode(&extracted.digest).unwrap_or_default(),
                size: artifact_bytes,
                storage_path: storage_path.clone(),
                metadata,
                deprecated: false,
                created_at: Utc::now(),
            };

            diesel::insert_into(artifact_versions::table)
                .values(&new_version)
                .execute(conn)
                .await?;

            audit(
                conn,
                Some(user_id),
                "upload",
                "artifact_version",
                Some(new_version.id),
                Some(serde_json::json!({
                    "artifact": artifact_name,
                    "version": version,
                    "app_version": app_version,
                })),
            )
            .await?;

            Ok(UploadedArtifact {
                artifact: artifact_name.clone(),
                version: version.clone(),
                owner: owner.to_string(),
            })
        })
        .await;

    match result {
        Ok(ok) => {
            tracing::Span::current()
                .record("artifact.name", ok.artifact.as_str())
                .record("artifact.version", ok.version.as_str());

            metrics::counter!(METRIC_UPLOADS_TOTAL, "owner" => owner.to_string()).increment(1);
            Ok(ok)
        }
        Err(e) => {
            // DB insert failed — release the quota reservation and clean up the file.
            quota::release_quota(state, user_id, artifact_bytes).await;
            let _ = tokio::fs::remove_file(
                FsPath::new(&state.config.charts_storage_path).join(&storage_path),
            )
            .await;
            Err(e)
        }
    }
}

// ── List Artifacts ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// Public artifact list item — extends `Artifact` with the owner's username so the
/// Explore UI can construct install URLs without a second round-trip.
#[derive(Serialize)]
pub struct PublicArtifact {
    #[serde(flatten)]
    pub artifact: Artifact,
    pub owner_username: String,
}

#[derive(QueryableByName)]
struct ArtifactWithUsername {
    #[diesel(embed)]
    artifact: Artifact,
    #[diesel(sql_type = diesel::sql_types::Text)]
    owner_username: String,
}

/// `GET /api/artifacts` — public artifact index with optional search
pub async fn list_artifacts(
    state: State<AppState>,
    RlsConn(mut conn): RlsConn,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<PublicArtifact>>, AppError> {
    let _state = &state.0;
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (params.page.unwrap_or(1) - 1) * per_page;

    let results: Vec<PublicArtifact> = if let Some(q) = params.q.as_deref() {
        diesel::sql_query(
            "
            SELECT a.*, u.username
            FROM artifacts a
            JOIN users u ON a.owner_id = u.id
            WHERE a.is_private = false AND (a.name % $1 OR a.description % $1)
            ORDER BY similarity(a.name, $1) DESC
            LIMIT $2 OFFSET $3
        ",
        )
        .bind::<diesel::sql_types::Text, _>(q)
        .bind::<diesel::sql_types::BigInt, _>(per_page)
        .bind::<diesel::sql_types::BigInt, _>(offset)
        .load::<ArtifactWithUsername>(&mut conn)
        .await?
        .into_iter()
        .map(|r| PublicArtifact {
            artifact: r.artifact,
            owner_username: r.owner_username,
        })
        .collect()
    } else {
        artifacts::table
            .inner_join(users::table.on(users::id.eq(artifacts::owner_id)))
            .filter(artifacts::is_private.eq(false))
            .select((Artifact::as_select(), users::username))
            .order(artifacts::download_count.desc())
            .limit(per_page)
            .offset(offset)
            .load::<(Artifact, String)>(&mut conn)
            .await?
            .into_iter()
            .map(|(artifact, owner_username)| PublicArtifact {
                artifact,
                owner_username,
            })
            .collect()
    };

    Ok(Json(results))
}

/// `GET /api/artifacts/:owner` — all public artifacts for a given user
pub async fn list_user_artifacts(
    state: State<AppState>,
    RlsConn(mut conn): RlsConn,
    Path(owner): Path<String>,
) -> Result<Json<Vec<Artifact>>, AppError> {
    let _state = &state.0;
    let user_id: Uuid = users::table
        .filter(users::username.eq(&owner))
        .select(users::id)
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("User '{owner}' not found")))?;

    let results = artifacts::table
        .filter(artifacts::owner_id.eq(user_id))
        .filter(artifacts::is_private.eq(false))
        .select(Artifact::as_select())
        .load(&mut conn)
        .await?;

    Ok(Json(results))
}

// ── Versions ──────────────────────────────────────────────────────────────────

/// `GET /api/artifacts/:owner/:artifact_name` — list all versions for an artifact
pub async fn list_artifact_versions(
    state: State<AppState>,
    RlsConn(mut conn): RlsConn,
    Path((owner, artifact_name)): Path<(String, String)>,
) -> Result<Json<Vec<ArtifactVersion>>, AppError> {
    let _state = &state.0;

    let user_id: Uuid = users::table
        .filter(users::username.eq(&owner))
        .select(users::id)
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("User '{owner}' not found")))?;

    let artifact: Artifact = artifacts::table
        .filter(artifacts::owner_id.eq(user_id))
        .filter(artifacts::name.eq(&artifact_name))
        .select(Artifact::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("Artifact '{artifact_name}' not found")))?;

    let versions = artifact_versions::table
        .filter(artifact_versions::artifact_id.eq(&artifact.id))
        .select(ArtifactVersion::as_select())
        .order(artifact_versions::created_at.desc())
        .load(&mut conn)
        .await?;

    Ok(Json(versions))
}

// ── Download ──────────────────────────────────────────────────────────────────

/// `GET /api/artifacts/:owner/:artifact_name/:version/download`
pub async fn download_artifact(
    state: State<AppState>,
    RlsConn(mut conn): RlsConn,
    Path((owner, artifact_name, version)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let state = &state.0;

    let user_id: Uuid = users::table
        .filter(users::username.eq(&owner))
        .select(users::id)
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("User '{owner}' not found")))?;

    let artifact: Artifact = artifacts::table
        .filter(artifacts::owner_id.eq(user_id))
        .filter(artifacts::name.eq(&artifact_name))
        .select(Artifact::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("Artifact '{artifact_name}' not found")))?;

    let av: ArtifactVersion = artifact_versions::table
        .filter(artifact_versions::artifact_id.eq(&artifact.id))
        .filter(artifact_versions::version.eq(&version))
        .select(ArtifactVersion::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("Version '{version}' not found")))?;

    let full_path = safe_join(
        FsPath::new(&state.config.charts_storage_path),
        &av.storage_path,
    )?;
    let bytes = tokio::fs::read(&full_path).await?;

    // Fire-and-forget download count increment (never fail the download on DB error).
    {
        metrics::counter!(METRIC_DOWNLOADS_TOTAL, "owner" => owner.clone(), "artifact" => artifact_name.clone()).increment(1);
        let artifact_id = artifact.id;
        let pool = state.db.clone();
        tokio::spawn(async move {
            if let Ok(mut c) = pool.get().await {
                let _ = diesel::update(artifacts::table.find(&artifact_id))
                    .set(artifacts::download_count.eq(artifacts::download_count + 1))
                    .execute(&mut c)
                    .await;
            }
        });
    }

    let content_disposition = format!("attachment; filename=\"{artifact_name}-{version}.tgz\"");

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/x-tar".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        content_disposition.parse().unwrap(),
    );

    Ok((headers, bytes))
}

// ── Delete Version ────────────────────────────────────────────────────────────

/// `DELETE /api/artifacts/:owner/:artifact_name/:version` — owner only.
///
/// When the last version of an artifact is removed the artifact record itself is also
/// deleted so it no longer appears in the owner's artifact list.
pub async fn delete_artifact_version(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path((owner, artifact_name, version)): Path<(String, String, String)>,
) -> Result<StatusCode, AppError> {
    let state = &state.0;
    if claims.username != owner {
        return Err(AppError::Forbidden(
            "Cannot delete from another user's namespace".into(),
        ));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let artifact: Artifact = artifacts::table
        .filter(artifacts::owner_id.eq(user_id))
        .filter(artifacts::name.eq(&artifact_name))
        .select(Artifact::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("Artifact '{artifact_name}' not found")))?;

    let av: ArtifactVersion = artifact_versions::table
        .filter(artifact_versions::artifact_id.eq(&artifact.id))
        .filter(artifact_versions::version.eq(&version))
        .select(ArtifactVersion::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("Version '{version}' not found")))?;

    let full_path = safe_join(
        FsPath::new(&state.config.charts_storage_path),
        &av.storage_path,
    )?;
    let freed_bytes = tokio::fs::metadata(&full_path)
        .await
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    if full_path.exists() {
        tokio::fs::remove_file(&full_path).await?;
    }

    diesel::delete(artifact_versions::table.filter(artifact_versions::id.eq(&av.id)))
        .execute(&mut conn)
        .await?;

    audit(
        &mut conn,
        Some(user_id),
        "delete_version",
        "artifact_version",
        Some(av.id),
        Some(serde_json::json!({
            "artifact": artifact.name,
            "version": version,
        })),
    )
    .await?;

    // If no versions remain, remove the artifact record too.
    let remaining: i64 = artifact_versions::table
        .filter(artifact_versions::artifact_id.eq(&artifact.id))
        .count()
        .get_result(&mut conn)
        .await?;

    if remaining == 0 {
        diesel::delete(artifacts::table.filter(artifacts::id.eq(&artifact.id)))
            .execute(&mut conn)
            .await?;
    }

    if freed_bytes > 0 {
        quota::release_quota(&state, user_id, freed_bytes).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/artifacts/:owner/:artifact_name` — purge all versions and the artifact record.
pub async fn purge_artifact(
    state: State<AppState>,
    Extension(claims): Extension<Claims>,
    RlsConn(mut conn): RlsConn,
    Path((owner, artifact_name)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let state = &state.0;
    if claims.username != owner {
        return Err(AppError::Forbidden(
            "Cannot delete from another user's namespace".into(),
        ));
    }

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;

    let artifact: Artifact = artifacts::table
        .filter(artifacts::owner_id.eq(user_id))
        .filter(artifacts::name.eq(&artifact_name))
        .select(Artifact::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("Artifact '{artifact_name}' not found")))?;

    let versions: Vec<ArtifactVersion> = artifact_versions::table
        .filter(artifact_versions::artifact_id.eq(&artifact.id))
        .select(ArtifactVersion::as_select())
        .load(&mut conn)
        .await?;

    let mut freed_bytes: i64 = 0;
    for av in &versions {
        match safe_join(
            FsPath::new(&state.config.charts_storage_path),
            &av.storage_path,
        ) {
            Ok(full_path) if full_path.exists() => {
                freed_bytes += tokio::fs::metadata(&full_path)
                    .await
                    .map(|m| m.len() as i64)
                    .unwrap_or(0);
                if let Err(e) = tokio::fs::remove_file(&full_path).await {
                    tracing::warn!(path = %full_path.display(), error = %e, "Failed to remove artifact file during purge");
                }
            }
            Err(e) => tracing::warn!(error = %e, "Skipping unsafe storage_path during purge"),
            _ => {}
        }
    }

    diesel::delete(
        artifact_versions::table.filter(artifact_versions::artifact_id.eq(&artifact.id)),
    )
    .execute(&mut conn)
    .await?;

    diesel::delete(artifacts::table.filter(artifacts::id.eq(&artifact.id)))
        .execute(&mut conn)
        .await?;

    audit(
        &mut conn,
        Some(user_id),
        "purge_artifact",
        "artifact",
        Some(artifact.id),
        Some(serde_json::json!({
            "name": artifact.name,
            "version_count": versions.len(),
        })),
    )
    .await?;

    if freed_bytes > 0 {
        quota::release_quota(&state, user_id, freed_bytes).await;
    }

    Ok(StatusCode::NO_CONTENT)
}

// ── Helm Repository Index ─────────────────────────────────────────────────────

#[derive(Serialize)]
struct IndexEntry {
    #[serde(rename = "apiVersion")]
    api_version: String,
    name: String,
    version: String,
    #[serde(rename = "appVersion", skip_serializing_if = "Option::is_none")]
    app_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    urls: Vec<String>,
    created: String,
    digest: String,
}

#[derive(Serialize)]
struct HelmIndex {
    #[serde(rename = "apiVersion")]
    api_version: String,
    entries: BTreeMap<String, Vec<IndexEntry>>,
    generated: String,
}

/// `GET /api/artifacts/:owner/index.yaml` — Helm repository index for `helm repo add`.
pub async fn artifact_repo_index(
    state: State<AppState>,
    RlsConn(mut conn): RlsConn,
    headers: axum::http::HeaderMap,
    Path(owner): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let _state = &state.0;

    let user_id: Uuid = users::table
        .filter(users::username.eq(&owner))
        .select(users::id)
        .first(&mut conn)
        .await
        .map_err(|_| AppError::NotFound(format!("User '{owner}' not found")))?;

    let owner_artifacts: Vec<Artifact> = artifacts::table
        .filter(artifacts::owner_id.eq(&user_id))
        .filter(artifacts::is_private.eq(false))
        .select(Artifact::as_select())
        .load(&mut conn)
        .await?;

    let proto = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let base = format!("{proto}://{host}");

    let mut entries: BTreeMap<String, Vec<IndexEntry>> = BTreeMap::new();

    for artifact in owner_artifacts {
        let versions: Vec<ArtifactVersion> = artifact_versions::table
            .filter(artifact_versions::artifact_id.eq(&artifact.id))
            .select(ArtifactVersion::as_select())
            .order(artifact_versions::created_at.desc())
            .load(&mut conn)
            .await?;

        let artifact_name = artifact.name.clone();
        let artifact_entries: Vec<IndexEntry> = versions
            .into_iter()
            .map(|v| {
                let url = format!(
                    "{base}/api/artifacts/{owner}/{name}/{ver}/download",
                    name = artifact_name,
                    ver = v.version,
                );
                IndexEntry {
                    api_version: "v2".to_string(),
                    name: artifact_name.clone(),
                    version: v.version,
                    app_version: v
                        .metadata
                        .get("app_version")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    description: v
                        .metadata
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    urls: vec![url],
                    created: v.created_at.to_rfc3339(),
                    digest: hex::encode(v.digest),
                }
            })
            .collect();

        if !artifact_entries.is_empty() {
            entries.insert(artifact_name, artifact_entries);
        }
    }

    let now = chrono::Utc::now().to_rfc3339();
    let index = HelmIndex {
        api_version: "v1".to_string(),
        entries,
        generated: now,
    };

    let yaml = yaml_serde::to_string(&index)
        .map_err(|e| AppError::Internal(format!("Failed to serialize index.yaml: {e}")))?;

    Ok(([(header::CONTENT_TYPE, "application/x-yaml")], yaml))
}
