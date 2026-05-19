use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::gitlab_repos;

#[derive(Queryable, Selectable, Serialize, Clone, Identifiable)]
#[diesel(table_name = gitlab_repos)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GitlabRepo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub gitlab_connection_id: Uuid,
    pub repo_owner: String,
    pub repo_name: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = gitlab_repos)]
pub struct NewGitlabRepo {
    pub id: Uuid,
    pub user_id: Uuid,
    pub gitlab_connection_id: Uuid,
    pub repo_owner: String,
    pub repo_name: String,
    pub created_at: DateTime<Utc>,
}

impl NewGitlabRepo {
    pub fn new(
        user_id: Uuid,
        gitlab_connection_id: Uuid,
        repo_owner: String,
        repo_name: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            user_id,
            gitlab_connection_id,
            repo_owner,
            repo_name,
            created_at: Utc::now(),
        }
    }
}

#[derive(AsChangeset)]
#[diesel(table_name = gitlab_repos)]
pub struct UpdateGitlabRepo {
    pub last_synced_at: Option<Option<DateTime<Utc>>>,
}
