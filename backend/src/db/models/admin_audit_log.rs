use chrono::Utc;
use diesel::prelude::*;
use serde::Serialize;
use uuid::Uuid;

use crate::schema::admin_audit_log;

#[allow(dead_code)]
#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = admin_audit_log)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct AdminAuditLog {
    pub id: String,
    pub admin_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = admin_audit_log)]
pub struct NewAdminAuditLog {
    pub id: String,
    pub admin_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

impl NewAdminAuditLog {
    pub fn new(
        admin_id: impl Into<String>,
        action: impl Into<String>,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            admin_id: admin_id.into(),
            action: action.into(),
            target_type: target_type.into(),
            target_id: target_id.into(),
            metadata_json: metadata.map(|v| v.to_string()),
            created_at: Utc::now().to_rfc3339(),
        }
    }
}
