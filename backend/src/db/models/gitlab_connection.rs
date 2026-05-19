use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::gitlab_connections;

#[derive(Queryable, Selectable, Serialize, Clone, Identifiable)]
#[diesel(table_name = gitlab_connections)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct GitlabConnection {
    pub id: Uuid,
    pub user_id: Uuid,
    pub gitlab_id: String,
    pub gitlab_username: String,
    #[serde(skip_serializing)]
    pub gitlab_access_token: String,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Insertable)]
#[diesel(table_name = gitlab_connections)]
pub struct NewGitlabConnection {
    pub id: Uuid,
    pub user_id: Uuid,
    pub gitlab_id: String,
    pub gitlab_username: String,
    pub gitlab_access_token: String,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl NewGitlabConnection {
    pub fn new(
        user_id: Uuid,
        gitlab_id: String,
        gitlab_username: String,
        gitlab_access_token: String,
        avatar_url: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            user_id,
            gitlab_id,
            gitlab_username,
            gitlab_access_token,
            avatar_url,
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(AsChangeset)]
#[diesel(table_name = gitlab_connections)]
pub struct UpdateGitlabConnection {
    pub gitlab_username: Option<String>,
    pub gitlab_access_token: Option<String>,
    pub avatar_url: Option<Option<String>>,
    pub updated_at: DateTime<Utc>,
}
