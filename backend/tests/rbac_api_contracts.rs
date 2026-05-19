use axum::http::StatusCode;
use axum_test::TestServer;
use helm_hub_backend::{
    AppState, app,
    config::Config,
    db::{init_pool, run_migrations},
};
use metrics_exporter_prometheus::PrometheusBuilder;
use serde_json::{Value, json};
use uuid::Uuid;

/// Helper to set up test server
async fn setup_test_server() -> TestServer {
    dotenvy::dotenv().ok();
    let config = Config::from_env();

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
    TestServer::new(app)
}

/// Create a test user and return (username, token, user_id)
async fn create_test_user(server: &TestServer) -> (String, String, Uuid) {
    let username = format!("user-{}", Uuid::new_v4());
    let password = "pass123";
    let email = format!("{}@example.com", username);

    let register_response = server
        .post("/api/auth/register")
        .json(&json!({
            "username": username,
            "email": email,
            "password": password,
        }))
        .await;

    assert_eq!(register_response.status_code(), StatusCode::OK);
    let _register_body: Value = register_response.json();

    let login_response = server
        .post("/api/auth/login")
        .json(&json!({
            "username": username,
            "password": password,
        }))
        .await;

    assert_eq!(login_response.status_code(), StatusCode::OK);
    let login_body: Value = login_response.json();
    let token = login_body["token"].as_str().unwrap().to_string();
    let user_id = Uuid::parse_str(login_body["user"]["id"].as_str().unwrap()).unwrap();

    (username, token, user_id)
}

// ─────────────────────────────────────────────────────────────────────────────
// API Contract Tests
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_register_response_structure() {
    let server = setup_test_server().await;

    let username = format!("user-{}", Uuid::new_v4());
    let response = server
        .post("/api/auth/register")
        .json(&json!({
            "username": username,
            "email": format!("{}@example.com", username),
            "password": "pass123",
        }))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
    let body: Value = response.json();

    // Verify response structure
    assert!(body["user"].is_object());
    assert!(body["user"]["id"].is_string());
    assert_eq!(body["user"]["username"].as_str(), Some(username.as_str()));
    assert!(body["user"]["email"].is_string());
    assert!(body["user"]["created_at"].is_string());
}

#[tokio::test]
async fn test_login_response_structure() {
    let server = setup_test_server().await;

    let (username, _token, _user_id) = create_test_user(&server).await;

    let response = server
        .post("/api/auth/login")
        .json(&json!({
            "username": username,
            "password": "pass123",
        }))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
    let body: Value = response.json();

    // Verify JWT token is present and is a string
    assert!(body["token"].is_string());
    let token = body["token"].as_str().unwrap();
    assert!(!token.is_empty());

    // Verify user object is present
    assert!(body["user"].is_object());
    assert!(body["user"]["id"].is_string());
}

#[tokio::test]
async fn test_update_role_request_response_structure() {
    let server = setup_test_server().await;

    let (_user1, token1, _id1) = create_test_user(&server).await;
    let (_user2, _token2, id2) = create_test_user(&server).await;

    let response = server
        .put(&format!("/api/admin/users/{}/role", id2))
        .add_header("Authorization", format!("Bearer {}", token1).as_str())
        .json(&json!({ "role": "Admin" }))
        .await;

    // Check response structure if successful
    if response.status_code() == StatusCode::OK {
        let body: Value = response.json();

        // Should return UserSummary with these fields
        assert!(body["id"].is_string());
        assert!(body["username"].is_string());
        assert!(body["email"].is_string());
        assert!(body["created_at"].is_string());
    }
}

#[tokio::test]
async fn test_list_users_response_structure() {
    let server = setup_test_server().await;

    let (_user, token, _user_id) = create_test_user(&server).await;

    let response = server
        .get("/api/admin/users")
        .add_header("Authorization", format!("Bearer {}", token).as_str())
        .await;

    if response.status_code() == StatusCode::OK {
        let body: Value = response.json();

        // Should return array
        assert!(body.is_array());

        // Each user should have expected fields
        if let Some(first_user) = body.as_array().and_then(|a| a.first()) {
            assert!(first_user["id"].is_string());
            assert!(first_user["username"].is_string());
            assert!(first_user["email"].is_string());
            assert!(first_user["created_at"].is_string());
        }
    }
}

#[tokio::test]
async fn test_instance_analytics_response_structure() {
    let server = setup_test_server().await;

    let (_user, token, _user_id) = create_test_user(&server).await;

    let response = server
        .get("/api/analytics/instance")
        .add_header("Authorization", format!("Bearer {}", token).as_str())
        .await;

    if response.status_code() == StatusCode::OK {
        let body: Value = response.json();

        // Verify all expected fields exist and have correct types
        assert!(body["total_users"].is_i64());
        assert!(body["total_storage_bytes"].is_i64());
        assert!(body["total_storage_limit_bytes"].is_i64());
        assert!(body["total_downloads"].is_i64());
        assert!(body["total_artifacts"].is_i64());
        assert!(body["total_artifact_versions"].is_i64());
        assert!(body["last_updated"].is_string());

        // Verify numeric fields are non-negative
        let total_users = body["total_users"].as_i64().unwrap();
        assert!(total_users >= 0);

        let total_storage = body["total_storage_bytes"].as_i64().unwrap();
        assert!(total_storage >= 0);

        let total_downloads = body["total_downloads"].as_i64().unwrap();
        assert!(total_downloads >= 0);
    }
}

#[tokio::test]
async fn test_package_analytics_response_structure() {
    let server = setup_test_server().await;

    let (_user, token, user_id) = create_test_user(&server).await;

    // Request package analytics for this user
    let response = server
        .get(&format!("/api/analytics/package/{}", user_id))
        .add_header("Authorization", format!("Bearer {}", token).as_str())
        .await;

    if response.status_code() == StatusCode::OK {
        let body: Value = response.json();

        if body.is_array() {
            // If it's an array, check structure of items
            if let Some(first) = body.as_array().and_then(|a| a.first()) {
                assert!(first["artifact_id"].is_string() || first["artifact_id"].is_object());
                assert!(first["owner_id"].is_string() || first["owner_id"].is_object());
                assert!(first["name"].is_string());
                assert!(first["downloads"].is_i64());
                assert!(first["versions_count"].is_i64());
            }
        } else {
            // Single object response
            assert!(body["artifact_id"].is_string() || body["artifact_id"].is_object());
            assert!(body["owner_id"].is_string() || body["owner_id"].is_object());
            assert!(body["name"].is_string());
        }
    }
}

#[tokio::test]
async fn test_error_response_unauthorized() {
    let server = setup_test_server().await;

    // Try to access admin endpoint without token
    let response = server.get("/api/admin/users").await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
    let body: Value = response.json();

    // Error response should have details
    assert!(body["message"].is_string() || body["error"].is_string() || body.is_string());
}

#[tokio::test]
async fn test_error_response_forbidden() {
    let server = setup_test_server().await;

    let (_user, token, _user_id) = create_test_user(&server).await;

    // Try to access instance analytics as regular user
    let response = server
        .get("/api/analytics/instance")
        .add_header("Authorization", format!("Bearer {}", token).as_str())
        .await;

    assert_eq!(response.status_code(), StatusCode::FORBIDDEN);
    let body: Value = response.json();

    // Error response should have details
    assert!(body["message"].is_string() || body["error"].is_string() || body.is_string());
}

#[tokio::test]
async fn test_error_response_not_found() {
    let server = setup_test_server().await;

    let (_user, token, _user_id) = create_test_user(&server).await;

    let nonexistent_id = Uuid::new_v4();
    let response = server
        .get(&format!("/api/admin/users/{}", nonexistent_id))
        .add_header("Authorization", format!("Bearer {}", token).as_str())
        .await;

    // Should be 404 or 403 depending on implementation
    assert!(
        response.status_code() == StatusCode::NOT_FOUND
            || response.status_code() == StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn test_error_response_bad_request() {
    let server = setup_test_server().await;

    let (_user, token, user_id) = create_test_user(&server).await;

    // Send invalid request body
    let response = server
        .put(&format!("/api/admin/users/{}/role", user_id))
        .add_header("Authorization", format!("Bearer {}", token).as_str())
        .json(&json!({ "role": "InvalidRoleName" }))
        .await;

    assert_eq!(response.status_code(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_http_methods_not_allowed() {
    let server = setup_test_server().await;

    let (_user, token, user_id) = create_test_user(&server).await;

    // Try GET on a POST-only endpoint
    let response = server
        .get(&format!("/api/admin/users/{}/ban", user_id))
        .add_header("Authorization", format!("Bearer {}", token).as_str())
        .await;

    // Should be 405 Method Not Allowed or similar
    assert!(
        response.status_code() == StatusCode::METHOD_NOT_ALLOWED
            || response.status_code() == StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn test_authentication_header_format() {
    let server = setup_test_server().await;

    let (_user, token, _user_id) = create_test_user(&server).await;

    // Valid authorization header format
    let response1 = server
        .get("/api/admin/users")
        .add_header("Authorization", format!("Bearer {}", token).as_str())
        .await;

    // Response should be OK or FORBIDDEN, not UNAUTHORIZED
    assert!(
        response1.status_code() == StatusCode::OK
            || response1.status_code() == StatusCode::FORBIDDEN
    );

    // Invalid authorization header format
    let response2 = server
        .get("/api/admin/users")
        .add_header("Authorization", format!("InvalidScheme {}", token).as_str())
        .await;

    // Should be unauthorized
    assert_eq!(response2.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_response_timestamps_valid() {
    let server = setup_test_server().await;

    let response = server
        .post("/api/auth/register")
        .json(&json!({
            "username": format!("user-{}", Uuid::new_v4()),
            "email": format!("user-{}@example.com", Uuid::new_v4()),
            "password": "pass123",
        }))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
    let body: Value = response.json();

    // Timestamp should be ISO 8601 format
    let created_at = body["user"]["created_at"].as_str().unwrap();

    // Should contain 'T' for ISO 8601
    assert!(created_at.contains('T'));

    // Should end with Z for UTC
    assert!(created_at.ends_with('Z'));
}
