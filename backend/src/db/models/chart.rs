use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::charts;

#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = charts)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Chart {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub description: Option<String>,
    pub home_url: Option<String>,
    pub icon_url: Option<String>,
    /// Stored as a JSON array string: `["keyword1","keyword2"]`
    pub keywords: Option<String>,
    pub is_private: i32,
    pub created_at: String,
    pub updated_at: String,
    pub download_count: i32,
}

#[derive(Debug, Insertable, Deserialize)]
#[diesel(table_name = charts)]
pub struct NewChart {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub description: Option<String>,
    pub home_url: Option<String>,
    pub icon_url: Option<String>,
    pub keywords: Option<String>,
    pub is_private: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl NewChart {
    pub fn new(owner_id: String, name: String, description: Option<String>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            owner_id,
            name,
            description,
            home_url: None,
            icon_url: None,
            keywords: None,
            is_private: 0,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

