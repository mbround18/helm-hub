use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::chart_versions;

#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = chart_versions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ChartVersion {
    pub id: String,
    pub chart_id: String,
    pub version: String,
    pub app_version: Option<String>,
    pub description: Option<String>,
    /// SHA-256 hex digest of the .tgz
    pub digest: String,
    /// Relative path: `{username}/{chart_name}/{version}.tgz`
    pub storage_path: String,
    pub chart_yaml: String,
    pub values_yaml: Option<String>,
    pub schema_json: Option<String>,
    pub deprecated: i32,
    pub created_at: String,
}

impl ChartVersion {
    pub fn is_deprecated(&self) -> bool {
        self.deprecated != 0
    }
}

#[derive(Debug, Insertable, Deserialize)]
#[diesel(table_name = chart_versions)]
pub struct NewChartVersion {
    pub id: String,
    pub chart_id: String,
    pub version: String,
    pub app_version: Option<String>,
    pub description: Option<String>,
    pub digest: String,
    pub storage_path: String,
    pub chart_yaml: String,
    pub values_yaml: Option<String>,
    pub schema_json: Option<String>,
    pub deprecated: i32,
    pub created_at: String,
}

impl NewChartVersion {
    pub fn new(
        chart_id: String,
        version: String,
        app_version: Option<String>,
        description: Option<String>,
        digest: String,
        storage_path: String,
        chart_yaml: String,
        values_yaml: Option<String>,
        schema_json: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            chart_id,
            version,
            app_version,
            description,
            digest,
            storage_path,
            chart_yaml,
            values_yaml,
            schema_json,
            deprecated: 0,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}
