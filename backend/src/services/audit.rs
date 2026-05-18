use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::{
    db::models::NewAuditLog,
    error::AppError,
    schema::audit_logs,
};

/// Records a state-changing action in the `audit_logs` table.
///
/// This is used to maintain an immutable, timestamped record of security-critical
/// and administrative actions for forensic analysis and compliance.
pub async fn audit(
    conn: &mut crate::db::DbConn,
    actor_id: Option<Uuid>,
    action: &str,
    target_type: &str,
    target_id: Option<Uuid>,
    metadata: Option<serde_json::Value>,
) -> Result<(), AppError> {
    let log = NewAuditLog::new(
        actor_id,
        action,
        target_type,
        target_id,
        metadata.unwrap_or(serde_json::json!({})),
    );

    diesel::insert_into(audit_logs::table)
        .values(&log)
        .execute(conn)
        .await?;

    Ok(())
}
