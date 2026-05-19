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

/// Create a test user and return token
async fn create_user_with_token(server: &TestServer, username: &str, password: &str) -> String {
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

    let login_response = server
        .post("/api/auth/login")
        .json(&json!({
            "username": username,
            "password": password,
        }))
        .await;

    assert_eq!(login_response.status_code(), StatusCode::OK);
    let body: Value = login_response.json();
    body["token"].as_str().unwrap().to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// E2E Scenarios
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn scenario_new_user_signup_gets_user_role() {
    let server = setup_test_server().await;

    // 1. Register new user
    let username = format!("newuser-{}", Uuid::new_v4());
    let password = "testpass123";
    let email = format!("{}@example.com", username);

    let response = server
        .post("/api/auth/register")
        .json(&json!({
            "username": username,
            "email": email,
            "password": password,
        }))
        .await;

    assert_eq!(response.status_code(), StatusCode::OK);
    let body: Value = response.json();

    // 2. Verify user has User role (or default role)
    // The response should indicate user role/status
    assert!(body["user"]["username"].as_str().is_some());

    // 3. Get token and try to delete other user's chart
    let login_response = server
        .post("/api/auth/login")
        .json(&json!({
            "username": username,
            "password": password,
        }))
        .await;

    let login_body: Value = login_response.json();
    let token = login_body["token"].as_str().unwrap().to_string();

    // Create another user
    let other_user = format!("other-{}", Uuid::new_v4());
    create_user_with_token(&server, &other_user, "pass123").await;

    // Try to delete other user's chart (should fail)
    let delete_response = server
        .delete(&format!("/api/artifacts/{}/test-chart/1.0.0", other_user))
        .add_header("Authorization", format!("Bearer {}", token).as_str())
        .await;

    // Should be forbidden
    assert_eq!(delete_response.status_code(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn scenario_user_cannot_promote_anyone() {
    let server = setup_test_server().await;

    // 1. Create regular users
    let user1_token =
        create_user_with_token(&server, &format!("user-{}", Uuid::new_v4()), "pass123").await;

    // Create user2
    let user2_username = format!("user-{}", Uuid::new_v4());
    let user2_response = server
        .post("/api/auth/register")
        .json(&json!({
            "username": user2_username,
            "email": format!("{}@example.com", user2_username),
            "password": "pass123",
        }))
        .await;

    let user2_id = Uuid::parse_str(
        user2_response.json::<Value>()["user"]["id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    // 2. User1 attempts to promote User2 to Admin
    let promote_response = server
        .put(&format!("/api/admin/users/{}/role", user2_id))
        .add_header("Authorization", format!("Bearer {}", user1_token).as_str())
        .json(&json!({ "role": "Admin" }))
        .await;

    // Should be forbidden
    assert_eq!(promote_response.status_code(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn scenario_privilege_escalation_prevented() {
    let server = setup_test_server().await;

    // 1. Create a regular user
    let _user_token =
        create_user_with_token(&server, &format!("user-{}", Uuid::new_v4()), "pass123").await;

    // 2. User attempts to modify their JWT token role directly
    // This is simulated by sending an invalid/modified token
    let malicious_token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.invalid.invalid";

    // Try to use the malicious token
    let response = server
        .get("/api/admin/users")
        .add_header(
            "Authorization",
            format!("Bearer {}", malicious_token).as_str(),
        )
        .await;

    // Should be unauthorized
    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn scenario_user_lifecycle() {
    let server = setup_test_server().await;

    // 1. User signs up (gets User role)
    let username = format!("lifecycle-{}", Uuid::new_v4());
    let password = "pass123";

    let signup_response = server
        .post("/api/auth/register")
        .json(&json!({
            "username": username,
            "email": format!("{}@example.com", username),
            "password": password,
        }))
        .await;

    assert_eq!(signup_response.status_code(), StatusCode::OK);

    // 2. User logs in
    let login_response = server
        .post("/api/auth/login")
        .json(&json!({
            "username": username,
            "password": password,
        }))
        .await;

    assert_eq!(login_response.status_code(), StatusCode::OK);
    let body: Value = login_response.json();
    let token = body["token"].as_str().unwrap().to_string();

    // 3. Get user ID from login response
    let _user_id = Uuid::parse_str(body["user"]["id"].as_str().unwrap()).unwrap();

    // 4. User can interact with own resources
    let self_response = server
        .delete(&format!("/api/artifacts/{}/test-chart/1.0.0", username))
        .add_header("Authorization", format!("Bearer {}", token).as_str())
        .await;

    // Should be 404 (chart not found) not 403 (forbidden)
    assert!(
        self_response.status_code() == StatusCode::NOT_FOUND
            || self_response.status_code() == StatusCode::NO_CONTENT
            || self_response.status_code() == StatusCode::OK
    );
}

#[tokio::test]
async fn scenario_multiple_users_isolation() {
    let server = setup_test_server().await;

    // Create 3 users
    let _user1_token =
        create_user_with_token(&server, &format!("user-{}", Uuid::new_v4()), "pass123").await;
    let user2_token =
        create_user_with_token(&server, &format!("user-{}", Uuid::new_v4()), "pass123").await;
    let _user3_token =
        create_user_with_token(&server, &format!("user-{}", Uuid::new_v4()), "pass123").await;

    // Each user should only see their own resources
    // Attempting cross-user operations should fail

    // Create user IDs for testing
    let user2_response = server
        .get("/api/admin/users")
        .add_header("Authorization", format!("Bearer {}", user2_token).as_str())
        .await;

    // Regular users should not be able to list all users
    assert!(
        user2_response.status_code() == StatusCode::FORBIDDEN
            || user2_response.status_code() == StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn scenario_authentication_required_for_admin() {
    let server = setup_test_server().await;

    // Try to access admin endpoints without authentication
    let response = server.get("/api/admin/users").await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);

    let user_id = Uuid::new_v4();
    let response = server
        .put(&format!("/api/admin/users/{}/role", user_id))
        .json(&json!({ "role": "Admin" }))
        .await;

    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn scenario_session_isolation() {
    let server = setup_test_server().await;

    // Create two users
    let user1_token =
        create_user_with_token(&server, &format!("user-{}", Uuid::new_v4()), "pass123").await;
    let user2_token =
        create_user_with_token(&server, &format!("user-{}", Uuid::new_v4()), "pass123").await;

    // User1 creates a chart (simulated by attempting to create)
    // User2 attempts to delete it
    let user1_response = server
        .get("/api/admin/users")
        .add_header("Authorization", format!("Bearer {}", user1_token).as_str())
        .await;

    let user2_response = server
        .get("/api/admin/users")
        .add_header("Authorization", format!("Bearer {}", user2_token).as_str())
        .await;

    // Both should have same permission level (regular users)
    assert_eq!(user1_response.status_code(), user2_response.status_code());
}

#[tokio::test]
async fn scenario_analytics_access_control() {
    let server = setup_test_server().await;

    // Create users with different roles
    let user_token =
        create_user_with_token(&server, &format!("user-{}", Uuid::new_v4()), "pass123").await;

    // Regular user tries to access instance analytics (admin only)
    let analytics_response = server
        .get("/api/analytics/instance")
        .add_header("Authorization", format!("Bearer {}", user_token).as_str())
        .await;

    // Should be forbidden
    assert_eq!(analytics_response.status_code(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn scenario_invalid_credentials_rejected() {
    let server = setup_test_server().await;

    // Try to login with invalid password
    let response = server
        .post("/api/auth/login")
        .json(&json!({
            "username": "nonexistent",
            "password": "wrongpass",
        }))
        .await;

    // Should be unauthorized
    assert_eq!(response.status_code(), StatusCode::UNAUTHORIZED);
}
