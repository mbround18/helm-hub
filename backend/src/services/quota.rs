use diesel::{prelude::*, sql_query, sql_types::BigInt};

use crate::{AppState, db::DbConn, error::AppError, schema::users};

/// Atomically reserve `bytes` of quota for `user_id`.
///
/// A single parameterized `UPDATE ... WHERE storage_usage_bytes + ? <= quota`
/// statement prevents TOCTOU races: two concurrent uploads cannot both succeed
/// if they would together exceed the limit.
///
/// Returns `AppError::QuotaExceeded` (413) when the row isn't updated (quota full).
pub fn reserve_quota(
    conn: &mut DbConn,
    user_id: &str,
    bytes: i64,
    global_default_bytes: i64,
) -> Result<(), AppError> {
    // Use sql_query with bind parameters to avoid any injection risk.
    let updated = sql_query(
        "UPDATE users \
         SET storage_usage_bytes = storage_usage_bytes + ? \
         WHERE id = ? \
           AND storage_usage_bytes + ? <= COALESCE(storage_quota_bytes, ?)",
    )
    .bind::<BigInt, _>(bytes)
    .bind::<diesel::sql_types::Text, _>(user_id)
    .bind::<BigInt, _>(bytes)
    .bind::<BigInt, _>(global_default_bytes)
    .execute(conn)
    .map_err(|e| AppError::Internal(format!("Quota update failed: {e}")))?;

    if updated == 0 {
        Err(AppError::QuotaExceeded)
    } else {
        Ok(())
    }
}

/// Release previously reserved quota bytes (on upload failure or chart deletion).
/// Fire-and-forget; saturates at 0 to avoid negative values.
pub fn release_quota(state: &AppState, user_id: &str, bytes: i64) {
    let pool = state.db.clone();
    let uid = user_id.to_string();
    tokio::spawn(async move {
        if let Ok(mut conn) = pool.get() {
            let _ = diesel::update(users::table.filter(users::id.eq(&uid)))
                .set(
                    users::storage_usage_bytes.eq(diesel::dsl::sql::<BigInt>(&format!(
                        "MAX(0, storage_usage_bytes - {bytes})"
                    ))),
                )
                .execute(&mut conn);
        }
    });
}
