use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::github_repos;

#[derive(Queryable, Selectable, Serialize, Clone, Identifiable)]
#[diesel(table_name = github_repos)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GithubRepo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub github_connection_id: Uuid,
    pub repo_owner: String,
    pub repo_name: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub sync_status: String,
    pub sync_started_at: Option<DateTime<Utc>>,
    pub sync_finished_at: Option<DateTime<Utc>>,
    pub sync_error: Option<String>,
    pub last_sync_report: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = github_repos)]
pub struct NewGithubRepo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub github_connection_id: Uuid,
    pub repo_owner: String,
    pub repo_name: String,
    pub created_at: DateTime<Utc>,
}

impl NewGithubRepo {
    pub fn new(
        user_id: Uuid,
        github_connection_id: Uuid,
        repo_owner: String,
        repo_name: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            github_connection_id,
            repo_owner,
            repo_name,
            created_at: Utc::now(),
        }
    }
}

#[derive(AsChangeset)]
#[diesel(table_name = github_repos)]
pub struct UpdateGithubRepo {
    pub last_synced_at: Option<Option<DateTime<Utc>>>,
    pub sync_status: Option<String>,
    pub sync_started_at: Option<Option<DateTime<Utc>>>,
    pub sync_finished_at: Option<Option<DateTime<Utc>>>,
    pub sync_error: Option<Option<String>>,
    pub last_sync_report: Option<Option<serde_json::Value>>,
}
