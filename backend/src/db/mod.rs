use axum::{extract::FromRequestParts, http::request::Parts};
use diesel::sql_query;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::{AsyncPgConnection, RunQueryDsl};

pub mod models;

use crate::{AppState, auth::jwt::Claims, error::AppError};

pub type DbPool = Pool<AsyncPgConnection>;
pub type DbConn = diesel_async::pooled_connection::deadpool::Object<AsyncPgConnection>;

/// A wrapper around `DbConn` that automatically sets the PostgreSQL `app.current_user_id`
/// and `app.is_admin` session variables if the request is authenticated.
/// This enforces Row Level Security (RLS) at the database level.
pub struct RlsConn(pub DbConn);

impl FromRequestParts<AppState> for RlsConn {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let mut conn = state
            .db
            .get()
            .await
            .map_err(|e| AppError::Pool(e.to_string()))?;

        // If the request has Claims (populated by require_auth middleware),
        // set the app.current_user_id and app.is_admin session variables for RLS.
        if let Some(claims) = parts.extensions.get::<Claims>() {
            set_current_user(&mut conn, &claims.sub, claims.is_admin)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to set RLS context: {e}")))?;
        }

        Ok(RlsConn(conn))
    }
}

pub fn init_pool(database_url: &str, pool_size: u32) -> DbPool {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
    Pool::builder(config)
        .max_size(pool_size as usize)
        .build()
        .expect("Failed to create database connection pool")
}

/// Sets the `app.current_user_id` and `app.is_admin` session variables in PostgreSQL.
/// This is used by Row Level Security (RLS) policies.
pub async fn set_current_user(
    conn: &mut AsyncPgConnection,
    user_id: &str,
    is_admin: bool,
) -> Result<(), diesel::result::Error> {
    sql_query(
        "SELECT set_config('app.current_user_id', $1, true), set_config('app.is_admin', $2, true)",
    )
    .bind::<diesel::sql_types::Text, _>(user_id)
    .bind::<diesel::sql_types::Text, _>(if is_admin { "true" } else { "false" })
    .execute(conn)
    .await?;
    Ok(())
}

/// Run all pending Diesel migrations embedded at compile time.
/// This uses a synchronous connection as diesel-async doesn't natively support migrations yet.
pub fn run_migrations(database_url: &str) {
    use diesel::Connection;
    use diesel::pg::PgConnection;
    use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
    let mut conn = PgConnection::establish(database_url)
        .expect("Failed to connect to database for migrations");

    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");
}
