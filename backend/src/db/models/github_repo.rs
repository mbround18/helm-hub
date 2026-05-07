use chrono::Utc;
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::github_repos;

#[derive(Queryable, Selectable, Serialize, Clone)]
#[diesel(table_name = github_repos)]
pub struct GithubRepo {
    pub id: String,
    pub user_id: String,
    pub github_connection_id: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub last_synced_at: Option<String>,
    pub created_at: String,
}

#[derive(Insertable)]
#[diesel(table_name = github_repos)]
pub struct NewGithubRepo {
    pub id: String,
    pub user_id: String,
    pub github_connection_id: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub created_at: String,
}

impl NewGithubRepo {
    pub fn new(
        user_id: String,
        github_connection_id: String,
        repo_owner: String,
        repo_name: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            github_connection_id,
            repo_owner,
            repo_name,
            created_at: Utc::now().to_rfc3339(),
        }
    }
}
