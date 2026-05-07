use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read},
    path::Path,
};
use tar::Archive;

#[derive(Debug)]
pub struct ExtractedChart {
    pub chart_yaml: String,
    pub values_yaml: Option<String>,
    pub schema_json: Option<String>,
    pub digest: String,
}

// ── Size limits ───────────────────────────────────────────────────────────────

/// Maximum decompressed size of a single metadata file (5 MiB).
const MAX_METADATA_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// Maximum total decompressed metadata across all files in one archive (20 MiB).
const MAX_TOTAL_METADATA_BYTES: u64 = 20 * 1024 * 1024;

// ── Path validation ───────────────────────────────────────────────────────────

/// Validates that a string is safe to use as a single filesystem path segment.
///
/// Rejects anything that could escape a directory: traversal sequences (`..`),
/// separators (`/`, `\`), null bytes, and characters outside the allowlist
/// `[A-Za-z0-9._-]`.  Also enforces sane length bounds.
pub fn validate_path_segment(s: &str, field: &str) -> io::Result<()> {
    if s.is_empty() || s.len() > 253 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} must be 1–253 characters"),
        ));
    }
    if s == ".." || s == "." {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} cannot be a relative directory reference"),
        ));
    }
    if s.contains('\0') || s.contains('/') || s.contains('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} contains illegal characters"),
        ));
    }
    // Allowlist: alphanumeric, hyphen, dot, underscore, plus (SemVer build metadata).
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '+'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{field} contains characters outside the allowed set [A-Za-z0-9._+-]"),
        ));
    }
    Ok(())
}

/// Reads at most `limit` bytes from `reader` into a `String`.
/// Returns an error if the content exceeds the limit.
fn read_limited<R: Read>(reader: &mut R, limit: u64, label: &str) -> io::Result<String> {
    let mut buf = String::new();
    // Read one extra byte so we can detect overruns without buffering the whole content.
    reader.take(limit + 1).read_to_string(&mut buf)?;
    if buf.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} exceeds the {limit}-byte decompression limit"),
        ));
    }
    Ok(buf)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Reads a `.tgz` Helm chart from `bytes`, computes its SHA-256, and extracts
/// `Chart.yaml`, `values.yaml`, and `values.schema.json`.
pub fn extract_chart_metadata(bytes: &[u8]) -> io::Result<ExtractedChart> {
    let hash = Sha256::digest(bytes);
    let digest = format!(
        "sha256:{}",
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );

    let cursor = io::Cursor::new(bytes);
    let gz = GzDecoder::new(cursor);
    let mut archive = Archive::new(gz);

    let mut chart_yaml: Option<String> = None;
    let mut values_yaml: Option<String> = None;
    let mut schema_json: Option<String> = None;
    let mut total_bytes: u64 = 0;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();

        // Only read files at depth 2: `<chartname>/<file>`.
        let components: Vec<_> = path.components().collect();
        if components.len() != 2 {
            continue;
        }

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        match file_name {
            "Chart.yaml" => {
                let content = read_limited(&mut entry, MAX_METADATA_FILE_BYTES, "Chart.yaml")?;
                total_bytes += content.len() as u64;
                if total_bytes > MAX_TOTAL_METADATA_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Archive total decompressed metadata exceeds the limit",
                    ));
                }
                chart_yaml = Some(content);
            }
            "values.yaml" => {
                let content = read_limited(&mut entry, MAX_METADATA_FILE_BYTES, "values.yaml")?;
                total_bytes += content.len() as u64;
                if total_bytes > MAX_TOTAL_METADATA_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Archive total decompressed metadata exceeds the limit",
                    ));
                }
                values_yaml = Some(content);
            }
            "values.schema.json" => {
                let content =
                    read_limited(&mut entry, MAX_METADATA_FILE_BYTES, "values.schema.json")?;
                total_bytes += content.len() as u64;
                if total_bytes > MAX_TOTAL_METADATA_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Archive total decompressed metadata exceeds the limit",
                    ));
                }
                schema_json = Some(content);
            }
            _ => {}
        }
    }

    let chart_yaml = chart_yaml.ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Chart.yaml not found in archive")
    })?;

    Ok(ExtractedChart {
        chart_yaml,
        values_yaml,
        schema_json,
        digest,
    })
}

/// Parses the `name` and `version` fields from raw `Chart.yaml` content.
pub fn parse_chart_yaml(content: &str) -> Option<(String, String, Option<String>, Option<String>)> {
    let doc: yaml_serde::Value = yaml_serde::from_str(content).ok()?;
    let name = doc["name"].as_str()?.to_string();
    let version = doc["version"].as_str()?.to_string();
    let app_version = doc["appVersion"].as_str().map(String::from);
    let description = doc["description"].as_str().map(String::from);
    Some((name, version, app_version, description))
}

/// Writes the chart `.tgz` to `{storage_root}/{owner}/{chart_name}/{version}.tgz`.
///
/// All path segments are validated before use; the final resolved path is
/// confirmed to be inside `storage_root` as a belt-and-suspenders check.
pub fn persist_chart(
    storage_root: &Path,
    owner: &str,
    chart_name: &str,
    version: &str,
    bytes: &[u8],
) -> io::Result<String> {
    // Validate every segment before it touches the filesystem.
    validate_path_segment(owner, "owner")?;
    validate_path_segment(chart_name, "chart name")?;
    validate_path_segment(version, "version")?;

    let dir = storage_root.join(owner).join(chart_name);
    std::fs::create_dir_all(&dir)?;
    let file_path = dir.join(format!("{version}.tgz"));

    // Belt-and-suspenders: confirm the resolved path stays inside storage_root.
    let canonical_root = storage_root.canonicalize()?;
    let canonical_dir = dir.canonicalize()?;
    if !canonical_dir.starts_with(&canonical_root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Resolved storage path escapes the storage root",
        ));
    }

    std::fs::write(&file_path, bytes)?;
    Ok(format!("{owner}/{chart_name}/{version}.tgz"))
}
