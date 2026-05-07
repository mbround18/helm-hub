use axum::{
    body::Bytes,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::Path as FsPath;
use uuid::Uuid;

use crate::{
    auth::jwt::Claims,
    db::models::{Chart, ChartVersion, NewChart, NewChartVersion},
    error::AppError,
    schema::{chart_versions, charts},
    services::{
        chart_extractor::{extract_chart_metadata, parse_chart_yaml, persist_chart},
        clamav::{scan_file, ScanOutcome},
    },
    AppState,
};

// ── Upload ────────────────────────────────────────────────────────────────────

/// `POST /api/charts/:owner`
///
/// Full upload flow:
///   1. Collect multipart bytes
///   2. Write to a unique temp file (so clamd can stream it off-disk)
///   3. ClamAV INSTREAM scan — infected → 403, clean → proceed
///   4. Extract Chart.yaml / values.yaml metadata
///   5. Persist to permanent storage and index in the database
///   6. Temp file is always cleaned up, even on error
pub async fn upload_chart(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(owner): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    if claims.username != owner {
        return Err(AppError::Forbidden("Cannot upload to another user's namespace".into()));
    }

    // ── 1. Collect multipart bytes ────────────────────────────────────────────
    let mut chart_bytes: Option<Bytes> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        if field.name() == Some("chart") {
            chart_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| AppError::BadRequest(e.to_string()))?,
            );
            break;
        }
    }
    let bytes =
        chart_bytes.ok_or_else(|| AppError::BadRequest("No `chart` field in multipart body".into()))?;

    // ── 2. Write to a temporary file for scanning ─────────────────────────────
    //
    // A unique filename avoids races if multiple uploads arrive concurrently.
    let temp_path = FsPath::new(&state.config.temp_upload_dir)
        .join(format!("helm-upload-{}.tgz", Uuid::new_v4()));

    tokio::fs::create_dir_all(&state.config.temp_upload_dir).await?;
    tokio::fs::write(&temp_path, &bytes).await?;

    // Wrap the rest in an async block so we can reliably remove the temp file
    // regardless of which branch we take.
    let result = run_security_gate_and_persist(
        &state,
        &claims,
        &owner,
        &bytes,
        &temp_path,
    )
    .await;

    // ── Always clean up the temp file ─────────────────────────────────────────
    if let Err(e) = tokio::fs::remove_file(&temp_path).await {
        // Non-fatal: log but don't mask the actual result
        tracing::warn!(path = %temp_path.display(), error = %e, "Failed to remove temp upload file");
    }

    result
}

/// Inner async function that can be `?`-propagated cleanly while still
/// guaranteeing the caller cleans up the temp file.
async fn run_security_gate_and_persist(
    state: &crate::AppState,
    claims: &Claims,
    owner: &str,
    bytes: &Bytes,
    temp_path: &std::path::Path,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    // ── 3. ClamAV virus scan ──────────────────────────────────────────────────
    if state.config.clamav_enabled {
        tracing::debug!(path = %temp_path.display(), "Running ClamAV scan");

        match scan_file(&state.config.clamd_socket, temp_path).await? {
            ScanOutcome::Clean => {
                tracing::info!(owner, "Chart scan passed — proceeding with upload");
            }
            ScanOutcome::Infected(virus) => {
                tracing::warn!(
                    owner,
                    virus_name = %virus,
                    "Infected chart upload blocked"
                );
                return Err(AppError::InfectedFile(format!(
                    "Upload rejected: virus/malware detected ({virus})"
                )));
            }
        }
    } else {
        tracing::warn!("CLAMAV_ENABLED=false — skipping virus scan (not for production)");
    }

    // ── 4. Extract Chart.yaml / values.yaml ──────────────────────────────────
    let extracted = extract_chart_metadata(bytes)
        .map_err(|e| AppError::BadRequest(format!("Invalid Helm chart archive: {e}")))?;

    let (chart_name, version, app_version, description) =
        parse_chart_yaml(&extracted.chart_yaml).ok_or_else(|| {
            AppError::BadRequest(
                "Chart.yaml missing required fields (name, version)".into(),
            )
        })?;

    // ── 5. Persist to permanent storage ───────────────────────────────────────
    let storage_root = FsPath::new(&state.config.charts_storage_path);
    let storage_path = persist_chart(storage_root, owner, &chart_name, &version, bytes)?;

    let mut conn = state.db.get()?;

    // Upsert the parent Chart row
    let chart: Chart = match charts::table
        .filter(charts::owner_id.eq(&claims.sub))
        .filter(charts::name.eq(&chart_name))
        .select(Chart::as_select())
        .first(&mut conn)
        .optional()?
    {
        Some(c) => c,
        None => {
            let new_chart =
                NewChart::new(claims.sub.clone(), chart_name.clone(), description.clone());
            diesel::insert_into(charts::table)
                .values(&new_chart)
                .execute(&mut conn)?;
            charts::table
                .filter(charts::id.eq(&new_chart.id))
                .select(Chart::as_select())
                .first(&mut conn)?
        }
    };

    // Guard against duplicate version
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

    // ── 6. Index in database ──────────────────────────────────────────────────
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

    diesel::insert_into(chart_versions::table)
        .values(&new_version)
        .execute(&mut conn)?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "message": "Chart uploaded successfully",
            "chart": chart_name,
            "version": version,
            "owner": owner,
        })),
    ))
}

// ── List Charts ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ChartSummary {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub description: Option<String>,
    pub latest_version: Option<String>,
}

/// `GET /api/charts` — public chart index with optional search
pub async fn list_charts(
    State(state): State<AppState>,
    Query(params): Query<SearchQuery>,
) -> Result<Json<Vec<Chart>>, AppError> {
    let mut conn = state.db.get()?;
    let per_page = params.per_page.unwrap_or(20).min(100);
    let offset = (params.page.unwrap_or(1) - 1) * per_page;

    let results = if let Some(q) = params.q.as_deref() {
        let pattern = format!("%{q}%");
        charts::table
            .filter(charts::is_private.eq(0))
            .filter(
                charts::name
                    .like(&pattern)
                    .or(charts::description.like(&pattern)),
            )
            .select(Chart::as_select())
            .limit(per_page)
            .offset(offset)
            .load(&mut conn)?
    } else {
        charts::table
            .filter(charts::is_private.eq(0))
            .select(Chart::as_select())
            .limit(per_page)
            .offset(offset)
            .load(&mut conn)?
    };

    Ok(Json(results))
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

    let full_path = FsPath::new(&state.config.charts_storage_path).join(&cv.storage_path);
    let bytes = tokio::fs::read(&full_path).await?;

    let content_disposition =
        format!("attachment; filename=\"{chart_name}-{version}.tgz\"");

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/x-tar".parse().unwrap());
    headers.insert(
        header::CONTENT_DISPOSITION,
        content_disposition.parse().unwrap(),
    );

    Ok((headers, bytes))
}

// ── Delete Version ────────────────────────────────────────────────────────────

/// `DELETE /api/charts/:owner/:chart_name/:version` — owner only
pub async fn delete_chart_version(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path((owner, chart_name, version)): Path<(String, String, String)>,
) -> Result<StatusCode, AppError> {
    if claims.username != owner {
        return Err(AppError::Forbidden("Cannot delete from another user's namespace".into()));
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

    // Remove from disk first, then database
    let full_path = FsPath::new(&state.config.charts_storage_path).join(&cv.storage_path);
    if full_path.exists() {
        tokio::fs::remove_file(&full_path).await?;
    }

    diesel::delete(chart_versions::table.filter(chart_versions::id.eq(&cv.id)))
        .execute(&mut conn)?;

    Ok(StatusCode::NO_CONTENT)
}
