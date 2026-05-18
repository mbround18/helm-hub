use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::artifacts;

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Identifiable, QueryableByName)]
#[diesel(table_name = artifacts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Artifact {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub r#type: String,
    pub description: Option<String>,
    pub metadata: serde_json::Value,
    pub is_private: bool,
    pub download_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable, Deserialize)]
#[diesel(table_name = artifacts)]
pub struct NewArtifact {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub r#type: String,
    pub description: Option<String>,
    pub metadata: serde_json::Value,
    pub is_private: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl NewArtifact {
    pub fn new(
        owner_id: Uuid,
        name: String,
        artifact_type: String,
        description: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            owner_id,
            name,
            r#type: artifact_type,
            description,
            metadata: serde_json::json!({}),
            is_private: false,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = artifacts)]
pub struct UpdateArtifact {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub metadata: Option<serde_json::Value>,
    pub is_private: Option<bool>,
    pub updated_at: DateTime<Utc>,
}

impl Default for UpdateArtifact {
    fn default() -> Self {
        Self {
            name: None,
            description: None,
            metadata: None,
            is_private: None,
            updated_at: Utc::now(),
        }
    }
}
