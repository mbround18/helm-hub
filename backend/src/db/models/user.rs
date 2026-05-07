use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::users;

#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    #[serde(skip_serializing)]
    pub totp_secret: Option<String>,
    pub totp_enabled: i32,
    pub is_admin: i32,
    pub banned_at: Option<String>,
    pub storage_usage_bytes: i64,
    pub storage_quota_bytes: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

impl User {
    pub fn is_totp_enabled(&self) -> bool {
        self.totp_enabled != 0
    }

    pub fn is_admin(&self) -> bool {
        self.is_admin != 0
    }

    pub fn is_banned(&self) -> bool {
        self.banned_at.is_some()
    }
}

#[derive(Debug, Insertable, Deserialize)]
#[diesel(table_name = users)]
pub struct NewUser {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub totp_secret: Option<String>,
    pub totp_enabled: i32,
    pub is_admin: i32,
    pub banned_at: Option<String>,
    pub storage_usage_bytes: i64,
    pub storage_quota_bytes: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

impl NewUser {
    pub fn new(username: String, email: String, password_hash: String) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            username,
            email,
            password_hash,
            totp_secret: None,
            totp_enabled: 0,
            is_admin: 0,
            banned_at: None,
            storage_usage_bytes: 0,
            storage_quota_bytes: None,
            created_at: now.clone(),
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
    pub totp_enabled: Option<i32>,
    pub updated_at: String,
}

impl Default for UpdateUser {
    fn default() -> Self {
        Self {
            email: None,
            password_hash: None,
            totp_secret: None,
            totp_enabled: None,
            updated_at: Utc::now().to_rfc3339(),
        }
    }
}

/// Changeset used by admin endpoints.
#[derive(Debug, AsChangeset)]
#[diesel(table_name = users)]
pub struct AdminUpdateUser {
    pub is_admin: Option<i32>,
    pub banned_at: Option<Option<String>>,
    pub storage_quota_bytes: Option<Option<i64>>,
    pub updated_at: String,
}
