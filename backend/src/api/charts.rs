use axum::{
    Extension, Json,
    body::Bytes,
    extract::{Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use diesel::prelude::*;
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
const MAX_CHARTS_PER_REQUEST: usize = 20;

// Metric names — defined as constants so they're greppable.
const METRIC_UPLOADS_TOTAL: &str = "helm_hub_chart_uploads_total";
const METRIC_UPLOAD_ERRORS_TOTAL: &str = "helm_hub_chart_upload_errors_total";
const METRIC_SCANS_TOTAL: &str = "helm_hub_clamav_scans_total";

#[derive(Serialize)]
pub(crate) struct UploadedChart {
    pub(crate) chart: String,
    pub(crate) version: String,
    pub(crate) owner: String,
}

#[derive(Serialize)]
struct FailedChart {
    error: String,
}

use crate::{
    AppState,
    auth::jwt::Claims,
    db::models::{Chart, ChartVersion, NewChart, NewChartVersion},
    error::AppError,
    schema::{chart_versions, charts},
    services::{
        chart_extractor::{extract_chart_metadata, parse_chart_yaml, persist_chart},
        clamav::{ScanOutcome, scan_file},
        quota,
    },
};

// ── Upload ────────────────────────────────────────────────────────────────────

/// `POST /api/charts/:owner`
///
/// Accepts one or more `chart` fields in a single multipart request.  Each
/// field is scanned, extracted, and persisted independently.  The response
/// always has HTTP 200 with `uploaded` and `failed` arrays so the caller can
/// surface per-file results without treating the whole request as an error.
///
/// curl example (single):
///   curl -X POST -H "Authorization: Bearer $TOKEN" \
///        -F "chart=@mychart-1.0.0.tgz" \
///        https://hub/api/charts/<owner>
///
/// curl example (multi):
///   curl -X POST -H "Authorization: Bearer $TOKEN" \
///        -F "chart=@chart1-1.0.0.tgz" -F "chart=@chart2-2.0.0.tgz" \
///        https://hub/api/charts/<owner>
#[tracing::instrument(skip(state, multipart), fields(owner = %owner))]
pub async fn upload_chart(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(owner): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
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
    if payloads.len() > MAX_CHARTS_PER_REQUEST {
        return Err(AppError::BadRequest(format!(
            "Maximum {MAX_CHARTS_PER_REQUEST} charts per request"
        )));
    }
    for payload in &payloads {
        if payload.len() > state.config.max_chart_file_bytes {
            return Err(AppError::BadRequest(format!(
                "Chart file exceeds the maximum size of {} MiB",
                state.config.max_chart_file_bytes / 1024 / 1024
            )));
        }
    }

    tokio::fs::create_dir_all(&state.config.temp_upload_dir).await?;

    // ── Process each chart independently ─────────────────────────────────────
    let mut uploaded: Vec<UploadedChart> = Vec::new();
    let mut failed: Vec<FailedChart> = Vec::new();

    for bytes in payloads {
        let temp_path = FsPath::new(&state.config.temp_upload_dir)
            .join(format!("helm-upload-{}.tgz", Uuid::new_v4()));

        tokio::fs::write(&temp_path, &bytes).await?;

        let outcome = scan_and_persist(&state, &claims.sub, &owner, &bytes, &temp_path).await;

        if let Err(e) = tokio::fs::remove_file(&temp_path).await {
            tracing::warn!(path = %temp_path.display(), error = %e, "Failed to remove temp upload file");
        }

        match outcome {
            Ok(ok) => uploaded.push(ok),
            Err(e) => {
                metrics::counter!(METRIC_UPLOAD_ERRORS_TOTAL, "reason" => "processing_error")
                    .increment(1);
                failed.push(FailedChart {
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

/// Runs the full security + persistence pipeline for a single chart file.
///
/// The caller is responsible for writing `bytes` to `temp_path` beforehand and
/// cleaning up the temp file afterwards regardless of the return value.
/// `user_id` is the UUID of the owning user; `owner` is their username (the
/// namespace charts are stored under).
#[tracing::instrument(
    skip(state, bytes),
    fields(owner, chart.name = tracing::field::Empty, chart.version = tracing::field::Empty)
)]
pub(crate) async fn scan_and_persist(
    state: &crate::AppState,
    user_id: &str,
    owner: &str,
    bytes: &Bytes,
    temp_path: &std::path::Path,
) -> Result<UploadedChart, AppError> {
    // ── ClamAV virus scan ─────────────────────────────────────────────────────
    if state.config.clamav_enabled {
        tracing::debug!(path = %temp_path.display(), "Running ClamAV scan");

        match scan_file(&state.config.clamd_socket, temp_path).await? {
            ScanOutcome::Clean => {
                tracing::info!(owner, "Chart scan passed — proceeding with upload");
                metrics::counter!(METRIC_SCANS_TOTAL, "result" => "clean").increment(1);
            }
            ScanOutcome::Infected(virus) => {
                tracing::warn!(owner, virus_name = %virus, "Infected chart upload blocked");
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

    let (chart_name, version, app_version, description) = parse_chart_yaml(&extracted.chart_yaml)
        .ok_or_else(|| {
        AppError::BadRequest("Chart.yaml missing required fields (name, version)".into())
    })?;

    // ── Quota check (atomic reserve before touching disk) ─────────────────────
    let chart_bytes = bytes.len() as i64;
    {
        let mut conn = state.db.get()?;
        quota::reserve_quota(
            &mut conn,
            user_id,
            chart_bytes,
            state.config.default_storage_quota_bytes,
        )?;
    }

    // ── Persist to permanent storage ──────────────────────────────────────────
    let storage_root = FsPath::new(&state.config.charts_storage_path);
    let storage_path = match persist_chart(storage_root, owner, &chart_name, &version, bytes) {
        Ok(p) => p,
        Err(e) => {
            // Release the reserved quota since the file wasn't written.
            quota::release_quota(state, user_id, chart_bytes);
            return Err(AppError::Io(e));
        }
    };

    let mut conn = state.db.get()?;

    let chart: Chart = match charts::table
        .filter(charts::owner_id.eq(user_id))
        .filter(charts::name.eq(&chart_name))
        .select(Chart::as_select())
        .first(&mut conn)
        .optional()?
    {
        Some(c) => c,
        None => {
            let new_chart =
                NewChart::new(user_id.to_string(), chart_name.clone(), description.clone());
            diesel::insert_into(charts::table)
                .values(&new_chart)
                .execute(&mut conn)?;
            charts::table
                .filter(charts::id.eq(&new_chart.id))
                .select(Chart::as_select())
                .first(&mut conn)?
        }
    };

    let exists = chart_versions::table
        .filter(chart_versions::chart_id.eq(&chart.id))
        .filter(chart_versions::version.eq(&version))
        .count()
        .get_result::<i64>(&mut conn)?
        > 0;

    if exists {
        return Err(AppError::Conflict(format!(
            "Version {version} of chart {chart_name} already exists"
        )));
    }

    // ── Index in database ─────────────────────────────────────────────────────
    let new_version = NewChartVersion::new(
        chart.id.clone(),
        version.clone(),
        app_version,
        description,
        extracted.digest,
        storage_path,
        extracted.chart_yaml,
        extracted.values_yaml,
        extracted.schema_json,
    );

    if let Err(e) = diesel::insert_into(chart_versions::table)
        .values(&new_version)
        .execute(&mut conn)
    {
        // DB insert failed — release the quota reservation and clean up the file.
        quota::release_quota(state, user_id, chart_bytes);
        let _ = tokio::fs::remove_file(
            FsPath::new(&state.config.charts_storage_path).join(&new_version.storage_path),
        )
        .await;
        return Err(AppError::from(e));
    }

    tracing::Span::current()
        .record("chart.name", &chart_name.as_str())
        .record("chart.version", &version.as_str());

    metrics::counter!(METRIC_UPLOADS_TOTAL, "owner" => owner.to_string()).increment(1);

    Ok(UploadedChart {
        chart: chart_name,
        version,
        owner: owner.to_string(),
    })
}

// ── List Charts ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// Public chart list item — extends `Chart` with the owner's username so the
/// Explore UI can construct install URLs without a second round-trip.
#[derive(Serialize)]
pub struct PublicChart {
    #[serde(flatten)]
    pub chart: Chart,
    pub owner_username: String,
}

/// `GET /api/charts` — public chart index with optional search
pub async fn list_charts(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<PublicChart>>, AppError> {
    use crate::schema::users;

    let mut conn = state.db.get()?;
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (params.page.unwrap_or(1) - 1) * per_page;

    let rows: Vec<(Chart, String)> = if let Some(q) = params.q.as_deref() {
        let pattern = format!("%{q}%");
        charts::table
            .inner_join(users::table.on(users::id.eq(charts::owner_id)))
            .filter(charts::is_private.eq(0))
            .filter(
                charts::name
                    .like(&pattern)
                    .or(charts::description.like(&pattern)),
            )
            .select((Chart::as_select(), users::username))
            .order(charts::download_count.desc())
            .limit(per_page)
            .offset(offset)
            .load(&mut conn)?
    } else {
        charts::table
            .inner_join(users::table.on(users::id.eq(charts::owner_id)))
            .filter(charts::is_private.eq(0))
            .select((Chart::as_select(), users::username))
            .order(charts::download_count.desc())
            .limit(per_page)
            .offset(offset)
            .load(&mut conn)?
    };

    Ok(Json(
        rows.into_iter()
            .map(|(chart, owner_username)| PublicChart {
                chart,
                owner_username,
            })
            .collect(),
    ))
}

/// `GET /api/charts/:owner` — all public charts for a given user
pub async fn list_user_charts(
    State(state): State<AppState>,
    Path(owner): Path<String>,
) -> Result<Json<Vec<Chart>>, AppError> {
    use crate::schema::users;

    let mut conn = state.db.get()?;

    let user_id: String = users::table
        .filter(users::username.eq(&owner))
        .select(users::id)
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("User '{owner}' not found")))?;

    let results = charts::table
        .filter(charts::owner_id.eq(user_id))
        .filter(charts::is_private.eq(0))
        .select(Chart::as_select())
        .load(&mut conn)?;

    Ok(Json(results))
}

// ── Versions ──────────────────────────────────────────────────────────────────

/// `GET /api/charts/:owner/:chart_name` — list all versions for a chart
pub async fn list_chart_versions(
    State(state): State<AppState>,
    Path((owner, chart_name)): Path<(String, String)>,
) -> Result<Json<Vec<ChartVersion>>, AppError> {
    use crate::schema::users;

    let mut conn = state.db.get()?;

    let user_id: String = users::table
        .filter(users::username.eq(&owner))
        .select(users::id)
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("User '{owner}' not found")))?;

    let chart: Chart = charts::table
        .filter(charts::owner_id.eq(user_id))
        .filter(charts::name.eq(&chart_name))
        .select(Chart::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("Chart '{chart_name}' not found")))?;

    let versions = chart_versions::table
        .filter(chart_versions::chart_id.eq(&chart.id))
        .select(ChartVersion::as_select())
        .order(chart_versions::created_at.desc())
        .load(&mut conn)?;

    Ok(Json(versions))
}

// ── Download ──────────────────────────────────────────────────────────────────

/// `GET /api/charts/:owner/:chart_name/:version/download`
pub async fn download_chart(
    State(state): State<AppState>,
    Path((owner, chart_name, version)): Path<(String, String, String)>,
) -> Result<impl IntoResponse, AppError> {
    use crate::schema::users;

    let mut conn = state.db.get()?;

    let user_id: String = users::table
        .filter(users::username.eq(&owner))
        .select(users::id)
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("User '{owner}' not found")))?;

    let chart: Chart = charts::table
        .filter(charts::owner_id.eq(user_id))
        .filter(charts::name.eq(&chart_name))
        .select(Chart::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("Chart '{chart_name}' not found")))?;

    let cv: ChartVersion = chart_versions::table
        .filter(chart_versions::chart_id.eq(&chart.id))
        .filter(chart_versions::version.eq(&version))
        .select(ChartVersion::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("Version '{version}' not found")))?;

    let full_path = safe_join(
        FsPath::new(&state.config.charts_storage_path),
        &cv.storage_path,
    )?;
    let bytes = tokio::fs::read(&full_path).await?;

    // Fire-and-forget download count increment (never fail the download on DB error).
    {
        let chart_id = chart.id.clone();
        let pool = state.db.clone();
        tokio::spawn(async move {
            if let Ok(mut c) = pool.get() {
                let _ = diesel::update(charts::table.find(&chart_id))
                    .set(charts::download_count.eq(charts::download_count + 1))
                    .execute(&mut c);
            }
        });
    }

    let content_disposition = format!("attachment; filename=\"{chart_name}-{version}.tgz\"");

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/x-tar".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        content_disposition.parse().unwrap(),
    );

    Ok((headers, bytes))
}

// ── Delete Version ────────────────────────────────────────────────────────────

/// `DELETE /api/charts/:owner/:chart_name/:version` — owner only.
///
/// When the last version of a chart is removed the chart record itself is also
/// deleted so it no longer appears in the owner's chart list.
pub async fn delete_chart_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((owner, chart_name, version)): Path<(String, String, String)>,
) -> Result<StatusCode, AppError> {
    if claims.username != owner {
        return Err(AppError::Forbidden(
            "Cannot delete from another user's namespace".into(),
        ));
    }

    let mut conn = state.db.get()?;

    let chart: Chart = charts::table
        .filter(charts::owner_id.eq(&claims.sub))
        .filter(charts::name.eq(&chart_name))
        .select(Chart::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("Chart '{chart_name}' not found")))?;

    let cv: ChartVersion = chart_versions::table
        .filter(chart_versions::chart_id.eq(&chart.id))
        .filter(chart_versions::version.eq(&version))
        .select(ChartVersion::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("Version '{version}' not found")))?;

    let full_path = safe_join(
        FsPath::new(&state.config.charts_storage_path),
        &cv.storage_path,
    )?;
    let freed_bytes = tokio::fs::metadata(&full_path)
        .await
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    if full_path.exists() {
        tokio::fs::remove_file(&full_path).await?;
    }

    diesel::delete(chart_versions::table.filter(chart_versions::id.eq(&cv.id)))
        .execute(&mut conn)?;

    // If no versions remain, remove the chart record too.
    let remaining: i64 = chart_versions::table
        .filter(chart_versions::chart_id.eq(&chart.id))
        .count()
        .get_result(&mut conn)?;

    if remaining == 0 {
        diesel::delete(charts::table.filter(charts::id.eq(&chart.id))).execute(&mut conn)?;
    }

    if freed_bytes > 0 {
        quota::release_quota(&state, &claims.sub, freed_bytes);
    }

    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /api/charts/:owner/:chart_name` — purge all versions and the chart record.
pub async fn purge_chart(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((owner, chart_name)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    if claims.username != owner {
        return Err(AppError::Forbidden(
            "Cannot delete from another user's namespace".into(),
        ));
    }

    let mut conn = state.db.get()?;

    let chart: Chart = charts::table
        .filter(charts::owner_id.eq(&claims.sub))
        .filter(charts::name.eq(&chart_name))
        .select(Chart::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("Chart '{chart_name}' not found")))?;

    let versions: Vec<ChartVersion> = chart_versions::table
        .filter(chart_versions::chart_id.eq(&chart.id))
        .select(ChartVersion::as_select())
        .load(&mut conn)?;

    let mut freed_bytes: i64 = 0;
    for cv in &versions {
        match safe_join(
            FsPath::new(&state.config.charts_storage_path),
            &cv.storage_path,
        ) {
            Ok(full_path) if full_path.exists() => {
                freed_bytes += tokio::fs::metadata(&full_path)
                    .await
                    .map(|m| m.len() as i64)
                    .unwrap_or(0);
                if let Err(e) = tokio::fs::remove_file(&full_path).await {
                    tracing::warn!(path = %full_path.display(), error = %e, "Failed to remove chart file during purge");
                }
            }
            Err(e) => tracing::warn!(error = %e, "Skipping unsafe storage_path during purge"),
            _ => {}
        }
    }

    diesel::delete(chart_versions::table.filter(chart_versions::chart_id.eq(&chart.id)))
        .execute(&mut conn)?;

    diesel::delete(charts::table.filter(charts::id.eq(&chart.id))).execute(&mut conn)?;

    if freed_bytes > 0 {
        quota::release_quota(&state, &claims.sub, freed_bytes);
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

/// `GET /api/charts/:owner/index.yaml` — Helm repository index for `helm repo add`.
pub async fn chart_repo_index(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(owner): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    use crate::schema::users;

    let mut conn = state.db.get()?;

    let user_id: String = users::table
        .filter(users::username.eq(&owner))
        .select(users::id)
        .first(&mut conn)
        .map_err(|_| AppError::NotFound(format!("User '{owner}' not found")))?;

    let owner_charts: Vec<Chart> = charts::table
        .filter(charts::owner_id.eq(&user_id))
        .filter(charts::is_private.eq(0))
        .select(Chart::as_select())
        .load(&mut conn)?;

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

    for chart in owner_charts {
        let versions: Vec<ChartVersion> = chart_versions::table
            .filter(chart_versions::chart_id.eq(&chart.id))
            .select(ChartVersion::as_select())
            .order(chart_versions::created_at.desc())
            .load(&mut conn)?;

        let chart_name = chart.name.clone();
        let chart_entries: Vec<IndexEntry> = versions
            .into_iter()
            .map(|v| {
                let url = format!(
                    "{base}/api/charts/{owner}/{name}/{ver}/download",
                    name = chart_name,
                    ver = v.version,
                );
                IndexEntry {
                    api_version: "v2".to_string(),
                    name: chart_name.clone(),
                    version: v.version,
                    app_version: v.app_version,
                    description: v.description,
                    urls: vec![url],
                    created: v.created_at,
                    digest: v.digest,
                }
            })
            .collect();

        if !chart_entries.is_empty() {
            entries.insert(chart_name, chart_entries);
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
