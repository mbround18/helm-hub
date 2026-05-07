use chrono::Utc;
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::api_tokens;

#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = api_tokens)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ApiToken {
    pub id: String,
    pub user_id: String,
    pub description: String,
    #[serde(skip_serializing)]
    #[allow(dead_code)] // required for Diesel Queryable field ordering; never read directly
    pub token_hash: String,
    pub expires_at: String,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = api_tokens)]
pub struct NewApiToken {
    pub id: String,
    pub user_id: String,
    pub description: String,
    pub token_hash: String,
    pub expires_at: String,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

impl NewApiToken {
    pub fn new(user_id: String, description: String, token_hash: String, expires_at: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            description,
            token_hash,
            expires_at,
            last_used_at: None,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}

/// Changeset for updating last_used_at.
#[derive(Debug, AsChangeset)]
#[diesel(table_name = api_tokens)]
pub struct TouchApiToken {
    pub last_used_at: Option<String>,
}
