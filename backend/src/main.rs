use helm_hub_backend::{
    AppState, app,
    config::Config,
    db::{init_pool, run_migrations},
};
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let config = Config::from_env();
    let (_telemetry, prometheus_handle) =
        helm_hub_backend::telemetry::init("helm-hub", &config.log_format);

    config.print_summary();

    if let Err(e) = config.validate() {
        tracing::error!("Configuration validation failed: {e}");
        std::process::exit(1);
    }

    let pool = init_pool(&config.database_url, config.db_pool_size);

    run_migrations(&config.database_url);

    std::fs::create_dir_all(&config.charts_storage_path)
        .expect("Failed to create chart storage directory");

    let http_client = reqwest::Client::builder()
        .user_agent("helm-hub/1.0")
        .build()
        .expect("Failed to build HTTP client");

    let state = AppState {
        db: pool,
        config: config.clone(),
        metrics: prometheus_handle,
        http_client,
    };

    // ── Background Metrics ────────────────────────────────────────────────────
    tokio::spawn(helm_hub_backend::services::metrics::start_background_metrics(state.clone()));

    let app = app(state.clone());
    let mgmt_app = helm_hub_backend::mgmt_app(state);

    let public_addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("Invalid bind address");

    let mgmt_addr: SocketAddr = format!("{}:{}", config.host, config.mgmt_port)
        .parse()
        .expect("Invalid mgmt address");

    tracing::info!("Helm Hub public API listening on http://{public_addr}");
    tracing::info!("Helm Hub mgmt API listening on http://{mgmt_addr}");

    let public_listener = tokio::net::TcpListener::bind(public_addr).await.unwrap();
    let mgmt_listener = tokio::net::TcpListener::bind(mgmt_addr).await.unwrap();

    let public_server = axum::serve(
        public_listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal("public"));

    let mgmt_server = axum::serve(
        mgmt_listener,
        mgmt_app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal("mgmt"));

    tokio::select! {
        res = public_server => {
            if let Err(e) = res {
                tracing::error!("Public server error: {e}");
            }
        }
        res = mgmt_server => {
            if let Err(e) = res {
                tracing::error!("Mgmt server error: {e}");
            }
        }
    }
}

async fn shutdown_signal(name: &'static str) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received for {name} server, starting graceful shutdown...");
}
