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

/// Reads a `.tgz` Helm chart from `bytes`, computes its SHA-256, and extracts
/// `Chart.yaml`, `values.yaml`, and `values.schema.json`.
pub fn extract_chart_metadata(bytes: &[u8]) -> io::Result<ExtractedChart> {
    // SHA-256 over the raw archive bytes
    let digest = format!("sha256:{:x}", Sha256::digest(bytes));

    let cursor = io::Cursor::new(bytes);
    let gz = GzDecoder::new(cursor);
    let mut archive = Archive::new(gz);

    let mut chart_yaml: Option<String> = None;
    let mut values_yaml: Option<String> = None;
    let mut schema_json: Option<String> = None;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Helm charts have a top-level directory; we match on file_name only.
        match file_name {
            "Chart.yaml" => {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                chart_yaml = Some(content);
            }
            "values.yaml" => {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                values_yaml = Some(content);
            }
            "values.schema.json" => {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                schema_json = Some(content);
            }
            _ => {}
        }
    }

    let chart_yaml = chart_yaml
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Chart.yaml not found in archive"))?;

    Ok(ExtractedChart {
        chart_yaml,
        values_yaml,
        schema_json,
        digest,
    })
}

/// Parses the `name` and `version` fields from raw `Chart.yaml` content.
pub fn parse_chart_yaml(content: &str) -> Option<(String, String, Option<String>, Option<String>)> {
    let doc: serde_yaml::Value = serde_yaml::from_str(content).ok()?;
    let name = doc["name"].as_str()?.to_string();
    let version = doc["version"].as_str()?.to_string();
    let app_version = doc["appVersion"].as_str().map(String::from);
    let description = doc["description"].as_str().map(String::from);
    Some((name, version, app_version, description))
}

/// Writes the chart `.tgz` to `{storage_root}/{owner}/{chart_name}/{version}.tgz`.
pub fn persist_chart(
    storage_root: &Path,
    owner: &str,
    chart_name: &str,
    version: &str,
    bytes: &[u8],
) -> io::Result<String> {
    let dir = storage_root.join(owner).join(chart_name);
    std::fs::create_dir_all(&dir)?;
    let file_path = dir.join(format!("{version}.tgz"));
    std::fs::write(&file_path, bytes)?;
    // Return relative path
    Ok(format!("{owner}/{chart_name}/{version}.tgz"))
}
