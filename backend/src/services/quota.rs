use diesel::{prelude::*, sql_query, sql_types::BigInt};
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::{AppState, db::DbConn, error::AppError, schema::users};

/// Atomically reserve `bytes` of quota for `user_id`.
///
/// A single parameterized `UPDATE ... WHERE storage_usage_bytes + ? <= quota`
/// statement prevents TOCTOU races: two concurrent uploads cannot both succeed
/// if they would together exceed the limit.
///
/// Returns `AppError::QuotaExceeded` (413) when the row isn't updated (quota full).
pub async fn reserve_quota(
    conn: &mut DbConn,
    user_id: Uuid,
    bytes: i64,
    global_default_bytes: i64,
) -> Result<(), AppError> {
    // PostgreSQL uses $1, $2, etc. for parameters.
    let updated = sql_query(
        "UPDATE users \
         SET storage_usage_bytes = storage_usage_bytes + $1 \
         WHERE id = $2 \
           AND storage_usage_bytes + $1 <= COALESCE(storage_quota_bytes, $3)",
    )
    .bind::<BigInt, _>(bytes)
    .bind::<diesel::sql_types::Uuid, _>(user_id)
    .bind::<BigInt, _>(global_default_bytes)
    .execute(conn)
    .await
    .map_err(|e| AppError::Internal(format!("Quota update failed: {e}")))?;

    if updated == 0 {
        Err(AppError::QuotaExceeded)
    } else {
        Ok(())
    }
}

/// Release previously reserved quota bytes (on upload failure or chart deletion).
/// Fire-and-forget; saturates at 0 to avoid negative values.
pub async fn release_quota(state: &AppState, user_id: Uuid, bytes: i64) {
    let pool = state.db.clone();
    tokio::spawn(async move {
        if let Ok(mut conn) = pool.get().await {
            // Use GREATEST(0, ...) in PostgreSQL to saturate at 0.
            let _ = diesel::update(users::table.filter(users::id.eq(user_id)))
                .set(
                    users::storage_usage_bytes.eq(diesel::dsl::sql::<BigInt>(&format!(
                        "GREATEST(0, storage_usage_bytes - {bytes})"
                    ))),
                )
                .execute(&mut conn)
                .await;
        }
    });
}
