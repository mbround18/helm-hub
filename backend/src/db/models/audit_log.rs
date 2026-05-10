use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::audit_logs;

#[allow(dead_code)]
#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = audit_logs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct AuditLog {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = audit_logs)]
pub struct NewAuditLog {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub action: String,
    pub target_type: String,
    pub target_id: Option<Uuid>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl NewAuditLog {
    pub fn new(
        actor_id: Option<Uuid>,
        action: impl Into<String>,
        target_type: impl Into<String>,
        target_id: Option<Uuid>,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            actor_id,
            action: action.into(),
            target_type: target_type.into(),
            target_id,
            metadata,
            created_at: Utc::now(),
        }
    }
}
