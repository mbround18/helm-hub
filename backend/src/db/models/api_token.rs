use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::api_tokens;

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Identifiable)]
#[diesel(table_name = api_tokens)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ApiToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub description: String,
    #[serde(skip_serializing)]
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = api_tokens)]
pub struct NewApiToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub description: String,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl NewApiToken {
    pub fn new(
        user_id: Uuid,
        description: String,
        token_hash: String,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            description,
            token_hash,
            expires_at,
            last_used_at: None,
            created_at: Utc::now(),
        }
    }
}

/// Changeset for updating last_used_at.
#[derive(Debug, AsChangeset)]
#[diesel(table_name = api_tokens)]
pub struct TouchApiToken {
    pub last_used_at: Option<DateTime<Utc>>,
}
