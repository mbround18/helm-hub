use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::github_connections;

#[derive(Queryable, Selectable, Serialize, Clone, Identifiable)]
#[diesel(table_name = github_connections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GithubConnection {
    pub id: Uuid,
    pub user_id: Uuid,
    pub github_id: String,
    pub github_username: String,
    #[serde(skip_serializing)]
    pub github_access_token: String,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = github_connections)]
pub struct NewGithubConnection {
    pub id: Uuid,
    pub user_id: Uuid,
    pub github_id: String,
    pub github_username: String,
    pub github_access_token: String,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl NewGithubConnection {
    pub fn new(
        user_id: Uuid,
        github_id: String,
        github_username: String,
        github_access_token: String,
        avatar_url: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            github_id,
            github_username,
            github_access_token,
            avatar_url,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(AsChangeset)]
#[diesel(table_name = github_connections)]
pub struct UpdateGithubConnection {
    pub github_username: Option<String>,
    pub github_access_token: Option<String>,
    pub avatar_url: Option<Option<String>>,
    pub updated_at: DateTime<Utc>,
}
