use chrono::Utc;
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::github_connections;

#[derive(Queryable, Selectable, Serialize, Clone)]
#[diesel(table_name = github_connections)]
pub struct GithubConnection {
    pub id: String,
    pub user_id: String,
    pub github_id: String,
    pub github_username: String,
    #[serde(skip_serializing)]
    pub github_access_token: String,
    pub avatar_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Insertable)]
#[diesel(table_name = github_connections)]
pub struct NewGithubConnection {
    pub id: String,
    pub user_id: String,
    pub github_id: String,
    pub github_username: String,
    pub github_access_token: String,
    pub avatar_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl NewGithubConnection {
    pub fn new(
        user_id: String,
        github_id: String,
        github_username: String,
        github_access_token: String,
        avatar_url: Option<String>,
    ) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            github_id,
            github_username,
            github_access_token,
            avatar_url,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}
