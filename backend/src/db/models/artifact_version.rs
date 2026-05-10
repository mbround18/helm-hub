use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::artifact_versions;

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Identifiable)]
#[diesel(table_name = artifact_versions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ArtifactVersion {
    pub id: Uuid,
    pub artifact_id: Uuid,
    pub version: String,
    pub digest: Vec<u8>,
    pub size: i64,
    pub storage_path: String,
    pub metadata: serde_json::Value,
    pub deprecated: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable, Deserialize)]
#[diesel(table_name = artifact_versions)]
pub struct NewArtifactVersion {
    pub id: Uuid,
    pub artifact_id: Uuid,
    pub version: String,
    pub digest: Vec<u8>,
    pub size: i64,
    pub storage_path: String,
    pub metadata: serde_json::Value,
    pub deprecated: bool,
    pub created_at: DateTime<Utc>,
}

impl NewArtifactVersion {
    pub fn new(
        artifact_id: Uuid,
        version: String,
        digest: Vec<u8>,
        size: i64,
        storage_path: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            artifact_id,
            version,
            digest,
            size,
            storage_path,
            metadata: serde_json::json!({}),
            deprecated: false,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = artifact_versions)]
pub struct UpdateArtifactVersion {
    pub metadata: Option<serde_json::Value>,
    pub deprecated: Option<bool>,
}
