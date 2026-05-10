use diesel::prelude::*;
use diesel_async::RunQueryDsl;

use crate::{db::DbConn, error::AppError, schema::app_settings};

pub async fn get(conn: &mut DbConn, key: &str) -> Result<String, AppError> {
    app_settings::table
        .filter(app_settings::key.eq(key))
        .select(app_settings::value)
        .first(conn)
        .await
        .map_err(|_| AppError::Internal(format!("Setting '{key}' not found")))
}

pub async fn set(conn: &mut DbConn, key: &str, value: &str) -> Result<(), AppError> {
    let now = chrono::Utc::now();
    diesel::insert_into(app_settings::table)
        .values((
            app_settings::key.eq(key),
            app_settings::value.eq(value),
            app_settings::updated_at.eq(&now),
        ))
        .on_conflict(app_settings::key)
        .do_update()
        .set((
            app_settings::value.eq(value),
            app_settings::updated_at.eq(&now),
        ))
        .execute(conn)
        .await?;
    Ok(())
}

pub async fn signup_enabled(conn: &mut DbConn) -> bool {
    get(conn, "signup_enabled")
        .await
        .map(|v| v != "false")
        .unwrap_or(true)
}
