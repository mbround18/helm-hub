use diesel_async::pooled_connection::deadpool::Pool;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use diesel::sql_query;

pub mod models;

pub type DbPool = Pool<AsyncPgConnection>;
pub type DbConn = diesel_async::pooled_connection::deadpool::Object<AsyncPgConnection>;

pub fn init_pool(database_url: &str) -> DbPool {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
    Pool::builder(config)
        .max_size(16)
        .build()
        .expect("Failed to create database connection pool")
}

/// Sets the `app.current_user_id` session variable in PostgreSQL.
/// This is used by Row Level Security (RLS) policies.
#[allow(dead_code)]
pub async fn set_current_user(conn: &mut AsyncPgConnection, user_id: &str) -> Result<(), diesel::result::Error> {
    sql_query("SELECT set_config('app.current_user_id', $1, true)")
        .bind::<diesel::sql_types::Text, _>(user_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// Run all pending Diesel migrations embedded at compile time.
/// This uses a synchronous connection as diesel-async doesn't natively support migrations yet.
pub fn run_migrations(database_url: &str) {
    use diesel::Connection;
    use diesel::pg::PgConnection;
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
    let mut conn = PgConnection::establish(database_url)
        .expect("Failed to connect to database for migrations");

    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");
}
