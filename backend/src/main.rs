use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::{HeaderValue, Method, StatusCode},
    middleware,
    response::{Html, IntoResponse},
    routing::{any, delete, get, post},
};
use metrics_exporter_prometheus::PrometheusHandle;
use std::net::SocketAddr;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

mod api;
mod auth;
mod config;
mod db;
mod error;
mod schema;
mod services;
mod telemetry;

use config::Config;
use db::{DbPool, init_pool, run_migrations};

async fn not_found() -> impl IntoResponse {
    StatusCode::NOT_FOUND
}

async fn spa_index() -> impl IntoResponse {
    match tokio::fs::read_to_string("/app/static/index.html").await {
        Ok(html) => Html(html).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read SPA entrypoint: {err}"),
        )
            .into_response(),
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub config: Config,
    pub metrics: PrometheusHandle,
    pub http_client: reqwest::Client,
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let _telemetry = telemetry::init("helm-hub");

    let prometheus_handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .expect("Failed to install Prometheus recorder");

    let config = Config::from_env();
    let pool = init_pool(&config.database_url);

    {
        let mut conn = pool
            .get()
            .expect("Failed to get DB connection for migrations");
        run_migrations(&mut conn);
    }

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

    // ── CORS ──────────────────────────────────────────────────────────────────
    // Restrict to the configured frontend origin — not a wildcard.
    // Credentials are not used (JWT is in the Authorization header, not cookies)
    // so allow_credentials is intentionally omitted.
    let frontend_origin: HeaderValue = config
        .frontend_origin
        .parse()
        .expect("FRONTEND_ORIGIN must be a valid HTTP origin");

    let cors = CorsLayer::new()
        .allow_origin(frontend_origin)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::PUT,
            Method::PATCH,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ]);

    // ── K8s operational endpoints (unauthenticated, no CORS, no rate limit) ──
    // In production: bind on a separate internal port not reachable from ingress.
    let k8s_routes = Router::new()
        .route("/k8s/healthz", get(api::k8s::healthz))
        .route("/k8s/readyz", get(api::k8s::readyz))
        .route("/k8s/metrics", get(api::k8s::metrics_handler));

    // ── Public routes — rate limited, no auth required ────────────────────────
    let public_routes = Router::new()
        .route("/api/settings", get(api::settings::get_settings))
        .route("/api/auth/register", post(api::auth::register))
        .route("/api/auth/login", post(api::auth::login))
        .route(
            "/api/auth/github/callback",
            get(api::github::oauth_callback),
        )
        .route("/api/charts", get(api::charts::list_charts))
        .route("/api/charts/{owner}", get(api::charts::list_user_charts))
        .route(
            "/api/charts/{owner}/index.yaml",
            get(api::charts::chart_repo_index),
        )
        .route(
            "/api/charts/{owner}/{chart_name}",
            get(api::charts::list_chart_versions),
        )
        .route(
            "/api/charts/{owner}/{chart_name}/{version}/download",
            get(api::charts::download_chart),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            services::rate_limiter::rate_limit,
        ));

    // ── Admin routes — JWT or API token required + admin role ────────────────
    let admin_routes = Router::new()
        .route("/api/admin/users", get(api::admin::list_users))
        .route(
            "/api/admin/users/{id}/promote",
            post(api::admin::promote_user),
        )
        .route("/api/admin/users/{id}/ban", post(api::admin::ban_user))
        .route("/api/admin/users/{id}", delete(api::admin::purge_user))
        .route(
            "/api/admin/users/{id}/quota",
            axum::routing::put(api::admin::set_user_quota),
        )
        .route(
            "/api/admin/charts/{owner}/{chart_name}",
            delete(api::admin::delete_any_chart),
        )
        .route(
            "/api/admin/settings",
            axum::routing::put(api::admin::update_settings),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::admin::require_admin,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            services::rate_limiter::rate_limit,
        ));

    // ── Authenticated routes — JWT or API token required ─────────────────────
    let authed_routes = Router::new()
        .route("/api/auth/totp/setup", post(api::auth::totp_setup))
        .route("/api/auth/totp/enable", post(api::auth::totp_enable))
        .route("/api/charts/{owner}", post(api::charts::upload_chart))
        .route(
            "/api/charts/{owner}/{chart_name}",
            delete(api::charts::purge_chart),
        )
        .route(
            "/api/charts/{owner}/{chart_name}/{version}",
            delete(api::charts::delete_chart_version),
        )
        .route(
            "/api/tokens",
            get(api::tokens::list_tokens).post(api::tokens::create_token),
        )
        .route("/api/tokens/{id}", delete(api::tokens::delete_token))
        .route("/api/auth/github/url", get(api::github::oauth_url))
        .route(
            "/api/github/connection",
            get(api::github::get_connection).delete(api::github::delete_connection),
        )
        .route(
            "/api/github/repos",
            get(api::github::list_repos).post(api::github::add_repo),
        )
        .route("/api/github/repos/{id}", delete(api::github::remove_repo))
        .route("/api/github/repos/{id}/sync", post(api::github::sync_repo))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            services::rate_limiter::rate_limit,
        ))
        // Cap multipart uploads; JSON endpoints are already tiny.
        .layer(DefaultBodyLimit::max(config.max_upload_body_bytes));

    let spa_routes = Router::new()
        .route("/api", any(not_found))
        .route("/api/{*path}", any(not_found))
        .route("/k8s", any(not_found))
        .route("/k8s/{*path}", any(not_found))
        .nest_service("/assets", ServeDir::new("/app/static/assets"))
        .route_service("/favicon.svg", ServeFile::new("/app/static/favicon.svg"))
        .route_service("/icons.svg", ServeFile::new("/app/static/icons.svg"))
        .route("/", get(spa_index))
        .fallback(get(spa_index));

    // CORS is applied only to API routes, not k8s operational routes.
    let app = Router::new()
        .merge(k8s_routes)
        .merge(public_routes.layer(cors.clone()))
        .merge(authed_routes.layer(cors.clone()))
        .merge(admin_routes.layer(cors))
        .merge(spa_routes)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("Invalid bind address");

    tracing::info!("Helm Hub listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();

    // ConnectInfo is required by the rate limiter to get the real peer address.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
