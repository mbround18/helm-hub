pub mod api;
pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod schema;
pub mod services;
pub mod telemetry;
pub mod utils;

use axum::{
    Router,
    extract::OriginalUri,
    extract::DefaultBodyLimit,
    http::{HeaderValue, Method},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::{any, delete, get, post},
};
use config::Config;
use metrics_exporter_prometheus::PrometheusHandle;
use services::seo;
use tower_http::{
    compression::CompressionLayer,
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub db: db::DbPool,
    pub config: Config,
    pub metrics: PrometheusHandle,
    pub http_client: reqwest::Client,
}

async fn not_found() -> impl IntoResponse {
    axum::http::StatusCode::NOT_FOUND
}

const SEO_TITLE: &str = "__SEO_TITLE__";
const SEO_DESCRIPTION: &str = "__SEO_DESCRIPTION__";
const SEO_CANONICAL: &str = "__SEO_CANONICAL__";
const SEO_ROBOTS: &str = "__SEO_ROBOTS__";
const SEO_OG_TYPE: &str = "__SEO_OG_TYPE__";
const SEO_IMAGE: &str = "__SEO_IMAGE__";
const SEO_TWITTER_CARD: &str = "__SEO_TWITTER_CARD__";

struct SeoTags {
    title: String,
    description: String,
    canonical: String,
    robots: String,
    og_type: String,
    image: String,
    twitter_card: String,
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn apply_seo(template: &str, seo: &SeoTags) -> String {
    template
        .replace(SEO_TITLE, &escape_html_attr(&seo.title))
        .replace(SEO_DESCRIPTION, &escape_html_attr(&seo.description))
        .replace(SEO_CANONICAL, &escape_html_attr(&seo.canonical))
        .replace(SEO_ROBOTS, &escape_html_attr(&seo.robots))
        .replace(SEO_OG_TYPE, &escape_html_attr(&seo.og_type))
        .replace(SEO_IMAGE, &escape_html_attr(&seo.image))
        .replace(SEO_TWITTER_CARD, &escape_html_attr(&seo.twitter_card))
}

fn default_seo(path: &str, origin: &str) -> SeoTags {
    let canonical = format!("{}{}", origin.trim_end_matches('/'), path);
    SeoTags {
        title: "Helm Hub".into(),
        description: "Discover, install, and share Helm charts.".into(),
        canonical,
        robots: "index,follow".into(),
        og_type: "website".into(),
        image: "/icons.svg".into(),
        twitter_card: "summary".into(),
    }
}

async fn page_seo(state: &AppState, path: &str) -> SeoTags {
    if let Some(rest) = path.strip_prefix("/charts/")
        && let mut parts = rest.splitn(2, '/')
        && let (Some(owner), Some(chart)) = (parts.next(), parts.next())
        && let Ok(mut conn) = state.db.get().await
        && let Ok(chart_seo) = seo::chart_seo(&mut conn, owner, chart).await
    {
        return SeoTags {
            title: chart_seo.title,
            description: chart_seo.description,
            canonical: format!(
                "{}{}",
                state.config.frontend_origin.trim_end_matches('/'),
                chart_seo.canonical_path
            ),
            robots: "index,follow".into(),
            og_type: "article".into(),
            image: "/icons.svg".into(),
            twitter_card: "summary".into(),
        };
    }

    default_seo(path, &state.config.frontend_origin)
}

async fn spa_index(
    axum::extract::State(state): axum::extract::State<AppState>,
    OriginalUri(uri): OriginalUri,
) -> impl IntoResponse {
    let path = uri.path().to_string();
    let index = std::path::Path::new(&state.config.static_assets_path).join("index.html");
    match tokio::fs::read_to_string(index).await {
        Ok(html) => {
            let seo = page_seo(&state, &path).await;
            Html(apply_seo(&html, &seo)).into_response()
        }
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "Failed to read SPA entrypoint (static_assets_path: {}): {err}",
                state.config.static_assets_path
            ),
        )
            .into_response(),
    }
}

pub fn mgmt_app(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(api::k8s::healthz))
        .route("/readyz", get(api::k8s::readyz))
        .route("/metrics", get(api::k8s::metrics_handler))
        .with_state(state)
}

pub fn app(state: AppState) -> Router {
    let frontend_origin: HeaderValue = state
        .config
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

    let probe_routes = Router::new()
        .route("/healthz", get(api::k8s::healthz))
        .route("/readyz", get(api::k8s::readyz));

    let public_routes = Router::new()
        .route("/api/settings", get(api::settings::get_settings))
        .route("/api/auth/providers", get(api::auth_providers::get_public_settings))
        .route("/api/telemetry/faro", post(api::telemetry::faro_proxy))
        .route("/api/auth/register", post(api::auth::register))
        .route("/api/auth/login", post(api::auth::login))
        .route("/api/auth/refresh", post(api::auth::refresh))
        .route(
            "/api/auth/github/callback",
            get(api::github::oauth_callback),
        )
        .route(
            "/api/auth/gitlab/callback",
            get(api::gitlab::oauth_callback),
        )
        .route("/api/auth/sso/github/login", get(api::auth_providers::github_login))
        .route(
            "/api/auth/sso/github/callback",
            get(api::auth_providers::github_callback),
        )
        .route("/api/auth/sso/gitlab/login", get(api::auth_providers::gitlab_login))
        .route(
            "/api/auth/sso/gitlab/callback",
            get(api::auth_providers::gitlab_callback),
        )
        .route("/api/artifacts", get(api::artifacts::list_artifacts))
        .route(
            "/api/artifacts/{owner}",
            get(api::artifacts::list_user_artifacts),
        )
        .route(
            "/api/artifacts/{owner}/index.yaml",
            get(api::artifacts::artifact_repo_index),
        )
        .route(
            "/api/artifacts/{owner}/{chart_name}",
            get(api::artifacts::list_artifact_versions),
        )
        .route(
            "/api/artifacts/{owner}/{chart_name}/{version}/download",
            get(api::artifacts::download_artifact),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            services::rate_limiter::rate_limit,
        ));

    let admin_routes = Router::new()
        .route("/api/admin/users", get(api::admin::list_users))
        .route(
            "/api/admin/users/{id}/promote",
            post(api::admin::promote_user),
        )
        .route(
            "/api/admin/users/{id}/role",
            axum::routing::put(api::admin::update_user_role),
        )
        .route("/api/admin/users/{id}/ban", post(api::admin::ban_user))
        .route("/api/admin/users/{id}", delete(api::admin::purge_user))
        .route(
            "/api/admin/users/{id}/quota",
            axum::routing::put(api::admin::set_user_quota),
        )
        .route(
            "/api/admin/artifacts/{owner}/{chart_name}",
            delete(api::admin::delete_any_artifact),
        )
        .route(
            "/api/admin/settings",
            axum::routing::put(api::admin::update_settings),
        )
        .route(
            "/api/admin/auth/providers",
            get(api::auth_providers::get_admin_settings).put(api::auth_providers::update_admin_settings),
        )
        .route(
            "/api/admin/observability",
            get(api::admin::get_observability_settings).put(api::admin::update_observability_settings),
        )
        .route(
            "/api/analytics/instance",
            get(api::analytics::get_instance_analytics),
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

    let authed_routes = Router::new()
        .route("/api/auth/totp/setup", post(api::auth::totp_setup))
        .route("/api/auth/totp/enable", post(api::auth::totp_enable))
        .route(
            "/api/artifacts/{owner}",
            post(api::artifacts::upload_artifact),
        )
        .route(
            "/api/artifacts/{owner}/{chart_name}",
            delete(api::artifacts::purge_artifact),
        )
        .route(
            "/api/artifacts/{owner}/{chart_name}/{version}",
            delete(api::artifacts::delete_artifact_version),
        )
        .route(
            "/api/tokens",
            get(api::tokens::list_tokens).post(api::tokens::create_token),
        )
        .route("/api/tokens/{id}", delete(api::tokens::delete_token))
        .route("/api/auth/me", get(api::auth_providers::me))
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
        .route("/api/auth/gitlab/url", get(api::gitlab::oauth_url))
        .route(
            "/api/gitlab/connection",
            get(api::gitlab::get_connection).delete(api::gitlab::delete_connection),
        )
        .route(
            "/api/gitlab/repos",
            get(api::gitlab::list_repos).post(api::gitlab::add_repo),
        )
        .route("/api/gitlab/repos/{id}", delete(api::gitlab::remove_repo))
        .route("/api/gitlab/repos/{id}/sync", post(api::gitlab::sync_repo))
        .route(
            "/api/analytics/package/{owner}/{chart}",
            get(api::analytics::get_package_analytics),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            services::rate_limiter::rate_limit,
        ))
        .layer(DefaultBodyLimit::max(state.config.max_upload_body_bytes));

    let spa_routes = Router::new()
        .route("/api", any(not_found))
        .route("/api/{*path}", any(not_found))
        .nest_service(
            "/assets",
            ServeDir::new(format!("{}/assets", state.config.static_assets_path)),
        )
        .route_service(
            "/favicon.svg",
            ServeFile::new(format!("{}/favicon.svg", state.config.static_assets_path)),
        )
        .route_service(
            "/icons.svg",
            ServeFile::new(format!("{}/icons.svg", state.config.static_assets_path)),
        )
        .route("/", get(spa_index))
        .fallback(get(spa_index));

    Router::new()
        .merge(probe_routes)
        .merge(public_routes.layer(cors.clone()))
        .merge(authed_routes.layer(cors.clone()))
        .merge(admin_routes.layer(cors))
        .merge(spa_routes)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(add_security_headers))
        .with_state(state)
}

async fn add_security_headers(req: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    headers.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; frame-ancestors 'none'; object-src 'none';"
            .parse()
            .unwrap(),
    );
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert(
        "Strict-Transport-Security",
        "max-age=31536000; includeSubDomains".parse().unwrap(),
    );

    response
}
