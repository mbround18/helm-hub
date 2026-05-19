//! Syncs Helm chart releases from a linked GitLab repository.
//!
//! Expected release naming convention (chart-releaser standard):
//!   Release tag:  `{chart-name}-{semver}`          e.g. `audiobookshelf-0.1.11`
//!   Asset name:   `{chart-name}-{semver}.tgz`
//!
//! The sync fetches all releases from the GitLab API, downloads every `.tgz`
//! asset whose name can be parsed as a chart/version pair, and runs each
//! through the standard scan-and-persist pipeline.  Versions that already
//! exist in the database are silently skipped.

use serde::Deserialize;
use uuid::Uuid;

use crate::{
    AppState,
    api::artifacts::scan_and_persist,
    db::DbConn,
    db::models::GitlabConnection,
    error::AppError,
};

// ── GitLab API response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct GitlabRelease {
    assets: GitlabReleaseAssets,
}

#[derive(Deserialize)]
struct GitlabReleaseAssets {
    sources: Vec<GitlabAsset>,
}

#[derive(Deserialize)]
struct GitlabAsset {
    url: String,
    #[serde(default)]
    format: String,
}

// ── Public result type ────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct ChartSyncEntry {
    pub chart: String,
    pub version: String,
    pub status: &'static str, // "imported" | "skipped" | "failed"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

// ── Core sync logic ───────────────────────────────────────────────────────────

/// Downloads all releases from the linked GitLab repository, imports new
/// chart versions, and returns a list of entries.
pub async fn sync(
    state: &AppState,
    conn: &mut DbConn,
    connection: &GitlabConnection,
    owner: &str,
    repo: &str,
) -> Result<Vec<ChartSyncEntry>, AppError> {
    // Decrypt token
    let encrypted_token = &connection.gitlab_access_token;
    let access_token = crate::services::token_crypto::decrypt_token(
        encrypted_token,
        &state.config.token_encryption_key,
    )?;

    let user_id = connection.user_id;
    
    let releases = fetch_releases(
        &state.http_client,
        &access_token,
        owner,
        repo,
    )
    .await?;

    let mut entries: Vec<ChartSyncEntry> = Vec::new();

    for release in &releases {
        for asset in &release.assets.sources {
            // Only process helm chart tarballs.
            if !asset.format.ends_with("tgz") {
                continue;
            }

            // Extract the filename from the URL
            let filename = asset.url.split('/').next_back().unwrap_or("");
            
            let Some((chart_name, version)) = parse_asset_name(filename) else {
                tracing::debug!(asset = %filename, "Skipping unrecognised asset name");
                continue;
            };

            // Skip if this version is already stored.
            if version_exists(state, user_id, &chart_name, &version).await {
                entries.push(ChartSyncEntry {
                    chart: chart_name,
                    version,
                    status: "skipped",
                    message: Some("already imported".into()),
                });
                continue;
            }

            let entry = download_and_import(
                state,
                conn,
                &access_token,
                &asset.url,
                &chart_name,
                &version,
                user_id,
            )
            .await;

            entries.push(entry);
        }
    }

    Ok(entries)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parses `{chart-name}-{version}.tgz` → `(chart_name, version)`.
pub fn parse_asset_name(filename: &str) -> Option<(String, String)> {
    let stem = filename.strip_suffix(".tgz")?;
    let bytes = stem.as_bytes();

    let split_at = (0..bytes.len())
        .rev()
        .find(|&i| bytes[i] == b'-' && bytes.get(i + 1).is_some_and(|b| b.is_ascii_digit()))?;

    let name = &stem[..split_at];
    let ver = &stem[split_at + 1..];

    if name.is_empty() || ver.is_empty() {
        return None;
    }
    Some((name.to_string(), ver.to_string()))
}

async fn version_exists(state: &AppState, user_id: Uuid, chart_name: &str, version: &str) -> bool {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use crate::schema::{artifact_versions, artifacts};

    let Ok(mut conn) = state.db.get().await else {
        return false;
    };

    let count: i64 = artifacts::table
        .inner_join(artifact_versions::table.on(artifact_versions::artifact_id.eq(artifacts::id)))
        .filter(artifacts::owner_id.eq(user_id))
        .filter(artifacts::name.eq(chart_name))
        .filter(artifact_versions::version.eq(version))
        .count()
        .get_result(&mut conn)
        .await
        .unwrap_or(0);

    count > 0
}

async fn fetch_releases(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<GitlabRelease>, AppError> {
    // GitLab API uses URL-encoded project path
    let project_path = format!("{}/{}", owner, repo);
    // Simple URL encoding: replace '/' with '%2F'
    let encoded_path = project_path.replace('/', "%2F");
    let url = format!("https://gitlab.com/api/v4/projects/{encoded_path}/releases?per_page=100");
    
    let resp = client
        .get(&url)
        .header("PRIVATE-TOKEN", token)
        .header("User-Agent", "helm-hub/1.0")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("GitLab API error: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "GitLab API returned {status}: {body}"
        )));
    }

    resp.json::<Vec<GitlabRelease>>()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse GitLab releases: {e}")))
}

async fn download_and_import(
    state: &AppState,
    conn: &mut DbConn,
    token: &str,
    download_url: &str,
    chart_name: &str,
    version: &str,
    user_id: Uuid,
) -> ChartSyncEntry {
    let make_failed = |msg: String| ChartSyncEntry {
        chart: chart_name.to_string(),
        version: version.to_string(),
        status: "failed",
        message: Some(msg),
    };

    // Download the tarball.
    let resp = match state
        .http_client
        .get(download_url)
        .header("PRIVATE-TOKEN", token)
        .header("User-Agent", "helm-hub/1.0")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return make_failed(format!("Download failed: {e}")),
    };

    let bytes_vec = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => return make_failed(format!("Failed to read download body: {e}")),
    };

    // Write to a temp file (required by scan_and_persist for ClamAV).
    let temp_path = std::path::Path::new(&state.config.temp_upload_dir).join(format!(
        "gl-sync-{}-{}.tgz",
        chart_name.replace('/', "-"),
        version.replace('/', "-")
    ));

    if let Err(e) = tokio::fs::write(&temp_path, &bytes_vec).await {
        return make_failed(format!("Failed to write temp file: {e}"));
    }

    // Run through the standard scan-and-persist pipeline.
    match scan_and_persist(
        state,
        conn,
        &user_id.to_string(),
        chart_name,
        &bytes_vec,
        &temp_path,
    )
    .await
    {
        Ok(_) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            ChartSyncEntry {
                chart: chart_name.to_string(),
                version: version.to_string(),
                status: "imported",
                message: None,
            }
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            make_failed(format!("Import failed: {e}"))
        }
    }
}
