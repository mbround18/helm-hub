use axum::http::StatusCode;
use axum_test::TestServer;
use helm_hub_backend::{
    AppState, app,
    config::Config,
    db::{init_pool, run_migrations},
};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_auth_flow() {
    dotenvy::dotenv().ok();
    let config = Config::from_env();

    // Ensure migrations are run on the DB
    run_migrations(&config.database_url);

    let metrics = PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install prometheus recorder");

    let pool = init_pool(&config.database_url, config.db_pool_size);
    let state = AppState {
        db: pool,
        config,
        metrics,
        http_client: reqwest::Client::new(),
    };

    let app = app(state);
    let server = TestServer::new(app);

    let username = format!("testuser-{}", Uuid::new_v4());
    let email = format!("{}@example.com", username);
    let password = "password123";

    // 1. Register
    let response = server
        .post("/api/auth/register")
        .json(&json!({
            "username": username,
            "email": email,
            "password": password,
        }))
        .await;
    response.assert_status(StatusCode::OK);

    let body = response.json::<serde_json::Value>();
    assert_eq!(body["user"]["username"], username);

    // 2. Login
    let response = server
        .post("/api/auth/login")
        .json(&json!({
            "username": username,
            "password": password,
        }))
        .await;
    response.assert_status(StatusCode::OK);

    let body = response.json::<serde_json::Value>();
    assert!(body["token"].is_string());
    assert_eq!(body["user"]["username"], username);
}
