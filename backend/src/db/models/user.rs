use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::users;

#[derive(Debug, Clone, Queryable, QueryableByName, Selectable, Serialize, Identifiable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    #[serde(skip_serializing)]
    pub totp_secret: Option<String>,
    pub totp_enabled: bool,
    pub is_admin: bool,
    pub banned_at: Option<DateTime<Utc>>,
    pub storage_usage_bytes: i64,
    pub storage_quota_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn is_totp_enabled(&self) -> bool {
        self.totp_enabled
    }

    pub fn is_admin(&self) -> bool {
        self.is_admin
    }

    pub fn is_banned(&self) -> bool {
        self.banned_at.is_some()
    }
}

#[derive(Debug, Insertable, Deserialize)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub totp_secret: Option<String>,
    pub totp_enabled: bool,
    pub is_admin: bool,
    pub banned_at: Option<DateTime<Utc>>,
    pub storage_usage_bytes: i64,
    pub storage_quota_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl NewUser {
    pub fn new(username: String, email: String, password_hash: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            username,
            email,
            password_hash,
            totp_secret: None,
            totp_enabled: false,
            is_admin: false,
            banned_at: None,
            storage_usage_bytes: 0,
            storage_quota_bytes: None,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = users)]
pub struct UpdateUser {
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub totp_secret: Option<Option<String>>,
    pub totp_enabled: Option<bool>,
    pub updated_at: DateTime<Utc>,
}

impl Default for UpdateUser {
    fn default() -> Self {
        Self {
            email: None,
            password_hash: None,
            totp_secret: None,
            totp_enabled: None,
            updated_at: Utc::now(),
        }
    }
}

/// Changeset used by admin endpoints.
#[derive(Debug, AsChangeset)]
#[diesel(table_name = users)]
pub struct AdminUpdateUser {
    pub is_admin: Option<bool>,
    pub banned_at: Option<Option<DateTime<Utc>>>,
    pub storage_quota_bytes: Option<Option<i64>>,
    pub updated_at: DateTime<Utc>,
}
