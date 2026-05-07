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

use crate::{
    auth::jwt::Claims,
    db::models::{Chart, ChartVersion, NewChart, NewChartVersion},
    error::AppError,
    schema::{chart_versions, charts},
    services::chart_extractor::{extract_chart_metadata, parse_chart_yaml, persist_chart},
    AppState,
};

// ── Upload ────────────────────────────────────────────────────────────────────

/// `POST /api/charts/:owner`
///
/// Accepts a multipart field named `chart` containing a `.tgz` Helm package.
/// Authenticated user must match `:owner`.
pub async fn upload_chart(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(owner): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    if claims.username != owner {
        return Err(AppError::Forbidden("Cannot upload to another user's namespace".into()));
    }

    // Collect the uploaded bytes
    let mut chart_bytes: Option<Bytes> = None;
    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
        if field.name() == Some("chart") {
            chart_bytes = Some(field.bytes().await.map_err(|e| AppError::BadRequest(e.to_string()))?);
            break;
        }
    }

    let bytes = chart_bytes.ok_or_else(|| AppError::BadRequest("No `chart` field in multipart body".into()))?;

    // Extract metadata from the archive
    let extracted = extract_chart_metadata(&bytes)
        .map_err(|e| AppError::BadRequest(format!("Invalid Helm chart archive: {e}")))?;

    let (chart_name, version, app_version, description) =
        parse_chart_yaml(&extracted.chart_yaml)
            .ok_or_else(|| AppError::BadRequest("Chart.yaml missing required fields (name, version)".into()))?;

    let storage_root = FsPath::new(&state.config.charts_storage_path);
    let storage_path = persist_chart(storage_root, &owner, &chart_name, &version, &bytes)?;

    let mut conn = state.db.get()?;

    // Upsert the parent Chart record
    let chart: Chart = match charts::table
        .filter(charts::owner_id.eq(&claims.sub))
        .filter(charts::name.eq(&chart_name))
        .select(Chart::as_select())
        .first(&mut conn)
        .optional()?
    {
        Some(c) => c,
        None => {
            let new_chart = NewChart::new(claims.sub.clone(), chart_name.clone(), description.clone());
            diesel::insert_into(charts::table)
                .values(&new_chart)
                .execute(&mut conn)?;
            charts::table
                .filter(charts::id.eq(&new_chart.id))
                .select(Chart::as_select())
                .first(&mut conn)?
        }
    };

    // Check for duplicate version
    let exists: bool = chart_versions::table
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
