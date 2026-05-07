use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

mod api;
mod auth;
mod config;
mod db;
mod error;
mod schema;
mod services;

use config::Config;
use db::{init_pool, run_migrations, DbPool};

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub config: Config,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("helm_hub_backend=debug".parse().unwrap()),
        )
        .init();

    let config = Config::from_env();
    let pool = init_pool(&config.database_url);

    // Run migrations on startup
    {
        let mut conn = pool.get().expect("Failed to get DB connection for migrations");
        run_migrations(&mut conn);
    }

    // Ensure chart storage directory exists
    std::fs::create_dir_all(&config.charts_storage_path)
        .expect("Failed to create chart storage directory");

    let state = AppState { db: pool, config: config.clone() };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes — no auth required
    let public_routes = Router::new()
        .route("/api/auth/register", post(api::auth::register))
        .route("/api/auth/login", post(api::auth::login))
        .route("/api/charts", get(api::charts::list_charts))
        .route("/api/charts/:owner", get(api::charts::list_user_charts))
        .route("/api/charts/:owner/:chart_name", get(api::charts::list_chart_versions))
        .route(
            "/api/charts/:owner/:chart_name/:version/download",
            get(api::charts::download_chart),
        );

    // Authenticated routes — JWT required
    let authed_routes = Router::new()
        .route("/api/auth/totp/setup", post(api::auth::totp_setup))
        .route("/api/auth/totp/enable", post(api::auth::totp_enable))
        .route("/api/charts/:owner", post(api::charts::upload_chart))
        .route(
            "/api/charts/:owner/:chart_name/:version",
            delete(api::charts::delete_chart_version),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    let app = Router::new()
        .merge(public_routes)
        .merge(authed_routes)
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("Invalid bind address");

    tracing::info!("Helm Hub listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
