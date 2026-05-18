use axum::http::StatusCode;
use axum_test::TestServer;
use helm_hub_backend::{AppState, app, config::Config, db::init_pool};
use metrics_exporter_prometheus::PrometheusBuilder;

#[tokio::test]
async fn test_health_check() {
    dotenvy::dotenv().ok();
    let config = Config::from_env();
    let pool = init_pool(&config.database_url, config.db_pool_size);

    let metrics = PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install prometheus recorder");

    let state = AppState {
        db: pool,
        config,
        metrics,
        http_client: reqwest::Client::new(),
    };

    let app = app(state);
    let server = TestServer::new(app);

    let response = server.get("/healthz").await;
    response.assert_status(StatusCode::OK);
}
