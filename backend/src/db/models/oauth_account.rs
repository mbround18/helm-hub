use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::oauth_accounts;

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Identifiable)]
#[diesel(table_name = oauth_accounts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct OauthAccount {
    pub id: Uuid,
    pub provider: String,
    pub provider_account_id: String,
    pub user_id: Uuid,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = oauth_accounts)]
pub struct NewOauthAccount {
    pub id: Uuid,
    pub provider: String,
    pub provider_account_id: String,
    pub user_id: Uuid,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
