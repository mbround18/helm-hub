//! Syncs Helm chart releases from a linked GitHub repository.
//!
//! Expected release naming convention (chart-releaser standard):
//!   Release tag:  `{chart-name}-{semver}`          e.g. `audiobookshelf-0.1.11`
//!   Asset name:   `{chart-name}-{semver}.tgz`
//!
//! The sync fetches all releases from the GitHub API, downloads every `.tgz`
//! asset whose name can be parsed as a chart/version pair, and runs each
//! through the standard scan-and-persist pipeline.  Versions that already
//! exist in the database are silently skipped.

use axum::body::Bytes;
use chrono::Utc;
use diesel::prelude::*;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    AppState,
    api::charts::scan_and_persist,
    db::models::{GithubRepo, User},
    error::AppError,
    schema::{chart_versions, charts, github_repos},
};

// ── GitHub API response types ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    content_type: String,
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

#[derive(serde::Serialize)]
pub struct SyncReport {
    pub repo: String,
    pub entries: Vec<ChartSyncEntry>,
    pub synced_at: String,
}

// ── Core sync logic ───────────────────────────────────────────────────────────

/// Downloads all releases from the linked GitHub repository, imports new
/// chart versions, and returns a detailed report.
pub async fn sync_repo(
    state: &AppState,
    repo: &GithubRepo,
    access_token: &str,
    user: &User,
) -> Result<SyncReport, AppError> {
    let releases = fetch_releases(
        &state.http_client,
        access_token,
        &repo.repo_owner,
        &repo.repo_name,
    )
    .await?;

    let mut entries: Vec<ChartSyncEntry> = Vec::new();

    for release in &releases {
        for asset in &release.assets {
            // Only process helm chart tarballs.
            if !asset.name.ends_with(".tgz")
                || !matches!(
                    asset.content_type.as_str(),
                    "application/x-tar"
                        | "application/gzip"
                        | "application/x-gzip"
                        | "application/octet-stream"
                )
            {
                continue;
            }

            let Some((chart_name, version)) = parse_asset_name(&asset.name) else {
                tracing::debug!(asset = %asset.name, "Skipping unrecognised asset name");
                continue;
            };

            // Skip if this version is already stored.
            if version_exists(state, &user.id, &chart_name, &version) {
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
                access_token,
                &asset.browser_download_url,
                &asset.name,
                &chart_name,
                &version,
                user,
            )
            .await;

            entries.push(entry);
        }
    }

    // Update last_synced_at
    let mut conn = state.db.get()?;
    diesel::update(github_repos::table.find(&repo.id))
        .set(github_repos::last_synced_at.eq(Utc::now().to_rfc3339()))
        .execute(&mut conn)?;

    Ok(SyncReport {
        repo: format!("{}/{}", repo.repo_owner, repo.repo_name),
        entries,
        synced_at: Utc::now().to_rfc3339(),
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parses `{chart-name}-{version}.tgz` → `(chart_name, version)`.
///
/// Finds the rightmost `-` followed by a digit, treating everything before it
/// as the chart name and everything after (until `.tgz`) as the version.
/// This correctly handles chart names that contain hyphens (e.g. `my-chart`).
pub fn parse_asset_name(filename: &str) -> Option<(String, String)> {
    let stem = filename.strip_suffix(".tgz")?;
    let bytes = stem.as_bytes();

    let split_at = (0..bytes.len())
        .rev()
        .find(|&i| bytes[i] == b'-' && bytes.get(i + 1).map_or(false, |b| b.is_ascii_digit()))?;

    let name = &stem[..split_at];
    let ver = &stem[split_at + 1..];

    if name.is_empty() || ver.is_empty() {
        return None;
    }
    Some((name.to_string(), ver.to_string()))
}

fn version_exists(state: &AppState, user_id: &str, chart_name: &str, version: &str) -> bool {
    let Ok(mut conn) = state.db.get() else {
        return false;
    };

    let count: i64 = charts::table
        .inner_join(chart_versions::table.on(chart_versions::chart_id.eq(charts::id)))
        .filter(charts::owner_id.eq(user_id))
        .filter(charts::name.eq(chart_name))
        .filter(chart_versions::version.eq(version))
        .count()
        .get_result(&mut conn)
        .unwrap_or(0);

    count > 0
}

async fn fetch_releases(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<Vec<GithubRelease>, AppError> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=100");
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "helm-hub/1.0")
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("GitHub API error: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "GitHub API returned {status}: {body}"
        )));
    }

    resp.json::<Vec<GithubRelease>>()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to parse GitHub releases: {e}")))
}

async fn download_and_import(
    state: &AppState,
    token: &str,
    download_url: &str,
    asset_name: &str,
    chart_name: &str,
    version: &str,
    user: &User,
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
        .header("Authorization", format!("Bearer {token}"))
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
    let bytes = Bytes::from(bytes_vec);

    // Write to a temp file (required by scan_and_persist for ClamAV).
    let temp_path = std::path::Path::new(&state.config.temp_upload_dir)
        .join(format!("gh-sync-{}-{}.tgz", Uuid::new_v4(), asset_name));

    if let Err(e) = tokio::fs::write(&temp_path, &bytes).await {
        return make_failed(format!("Failed to write temp file: {e}"));
    }

    let result = scan_and_persist(state, &user.id, &user.username, &bytes, &temp_path).await;

    if let Err(e) = tokio::fs::remove_file(&temp_path).await {
        tracing::warn!(path = %temp_path.display(), error = %e, "Failed to remove sync temp file");
    }

    match result {
        Ok(uploaded) => ChartSyncEntry {
            chart: uploaded.chart,
            version: uploaded.version,
            status: "imported",
            message: None,
        },
        Err(e) => make_failed(e.to_string()),
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::parse_asset_name;

    #[test]
    fn simple_name() {
        assert_eq!(
            parse_asset_name("audiobookshelf-0.1.11.tgz"),
            Some(("audiobookshelf".into(), "0.1.11".into()))
        );
    }

    #[test]
    fn hyphenated_name() {
        assert_eq!(
            parse_asset_name("my-chart-1.0.0.tgz"),
            Some(("my-chart".into(), "1.0.0".into()))
        );
    }

    #[test]
    fn pre_release_version() {
        assert_eq!(
            parse_asset_name("chart-a-2.0.0-beta.1.tgz"),
            Some(("chart-a".into(), "2.0.0-beta.1".into()))
        );
    }

    #[test]
    fn not_a_tgz() {
        assert_eq!(parse_asset_name("checksums.txt"), None);
    }

    #[test]
    fn no_version() {
        assert_eq!(parse_asset_name("chart.tgz"), None);
    }
}
