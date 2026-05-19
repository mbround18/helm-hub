use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Redirect,
    Extension, Json,
};
use chrono::Utc;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{
        jwt::{Claims, encode_jwt},
        password::hash_password,
    },
    db::models::User,
    error::AppError,
    schema::users,
    services::{audit::audit, auth_providers},
};

#[derive(Serialize, Deserialize)]
struct OAuthState {
    provider: String,
    return_to: String,
    exp: i64,
}

fn encode_oauth_state(
    provider: &str,
    return_to: &str,
    secret: &str,
) -> Result<String, AppError> {
    let claims = OAuthState {
        provider: provider.to_string(),
        return_to: return_to.to_string(),
        exp: Utc::now().timestamp() + 600,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("OAuth state encoding failed: {e}")))
}

fn decode_oauth_state(state: &str, secret: &str) -> Result<OAuthState, AppError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    decode::<OAuthState>(
        state,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|d| d.claims)
    .map_err(|_| AppError::Unauthorized("Invalid or expired OAuth state".into()))
}

fn safe_return_to(raw: Option<&str>) -> &str {
    match raw {
        Some(s) if s.starts_with('/') && !s.starts_with("//") => s,
        _ => "/profile",
    }
}

#[derive(Serialize)]
pub struct AuthProvidersResponse {
    pub local_enabled: bool,
    pub github_enabled: bool,
    pub gitlab_enabled: bool,
}

#[derive(Serialize)]
pub struct AdminAuthProvidersResponse {
    pub local_enabled: bool,
    pub github: auth_providers::ProviderAdminSettings,
    pub gitlab: auth_providers::ProviderAdminSettings,
}

#[derive(Deserialize)]
pub struct UpdateAuthProvidersBody {
    pub local_enabled: Option<bool>,
    pub github_enabled: Option<bool>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub github_redirect_uri: Option<String>,
    pub gitlab_enabled: Option<bool>,
    pub gitlab_base_url: Option<String>,
    pub gitlab_client_id: Option<String>,
    pub gitlab_client_secret: Option<String>,
    pub gitlab_redirect_uri: Option<String>,
}

pub async fn get_public_settings(
    State(state): State<AppState>,
) -> Result<Json<AuthProvidersResponse>, AppError> {
    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
    Ok(Json(AuthProvidersResponse {
        local_enabled: auth_providers::bool_setting(
            &mut conn,
            auth_providers::KEY_LOCAL_ENABLED,
            true,
        )
        .await,
        github_enabled: auth_providers::bool_setting(
            &mut conn,
            auth_providers::KEY_GITHUB_ENABLED,
            false,
        )
        .await,
        gitlab_enabled: auth_providers::bool_setting(
            &mut conn,
            auth_providers::KEY_GITLAB_ENABLED,
            false,
        )
        .await,
    }))
}

pub async fn get_admin_settings(
    State(state): State<AppState>,
) -> Result<Json<AdminAuthProvidersResponse>, AppError> {
    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
    Ok(Json(AdminAuthProvidersResponse {
        local_enabled: auth_providers::bool_setting(
            &mut conn,
            auth_providers::KEY_LOCAL_ENABLED,
            true,
        )
        .await,
        github: auth_providers::ProviderAdminSettings {
            enabled: auth_providers::bool_setting(
                &mut conn,
                auth_providers::KEY_GITHUB_ENABLED,
                false,
            )
            .await,
            client_id: auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITHUB_CLIENT_ID)
                .await,
            client_secret_set: auth_providers::opt_setting(
                &mut conn,
                auth_providers::KEY_GITHUB_CLIENT_SECRET,
            )
            .await
            .is_some(),
            redirect_uri: auth_providers::opt_setting(
                &mut conn,
                auth_providers::KEY_GITHUB_REDIRECT_URI,
            )
            .await,
            base_url: None,
        },
        gitlab: auth_providers::ProviderAdminSettings {
            enabled: auth_providers::bool_setting(
                &mut conn,
                auth_providers::KEY_GITLAB_ENABLED,
                false,
            )
            .await,
            client_id: auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_CLIENT_ID)
                .await,
            client_secret_set: auth_providers::opt_setting(
                &mut conn,
                auth_providers::KEY_GITLAB_CLIENT_SECRET,
            )
            .await
            .is_some(),
            redirect_uri: auth_providers::opt_setting(
                &mut conn,
                auth_providers::KEY_GITLAB_REDIRECT_URI,
            )
            .await,
            base_url: auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_BASE_URL)
                .await,
        },
    }))
}

pub async fn update_admin_settings(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    crate::db::RlsConn(mut conn): crate::db::RlsConn,
    Json(body): Json<UpdateAuthProvidersBody>,
) -> Result<StatusCode, AppError> {
    let admin_id = Uuid::parse_str(&claims.sub).ok();

    if let Some(enabled) = body.local_enabled {
        auth_providers::set_bool(&mut conn, auth_providers::KEY_LOCAL_ENABLED, enabled).await?;
    }

    if let Some(enabled) = body.github_enabled {
        auth_providers::set_bool(&mut conn, auth_providers::KEY_GITHUB_ENABLED, enabled).await?;
    }
    if let Some(ref client_id) = body.github_client_id {
        auth_providers::set_opt_setting(
            &mut conn,
            auth_providers::KEY_GITHUB_CLIENT_ID,
            Some(client_id.as_str()),
        )
        .await?;
    }
    if let Some(ref redirect_uri) = body.github_redirect_uri {
        let trimmed = redirect_uri.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("http://") && !trimmed.starts_with("https://")
        {
            return Err(AppError::BadRequest(
                "github_redirect_uri must be an http/https URL".into(),
            ));
        }
        auth_providers::set_opt_setting(
            &mut conn,
            auth_providers::KEY_GITHUB_REDIRECT_URI,
            Some(trimmed),
        )
        .await?;
    }
    if body.github_client_secret.is_some() {
        auth_providers::set_secret(
            &mut conn,
            auth_providers::KEY_GITHUB_CLIENT_SECRET,
            body.github_client_secret.as_deref(),
            &state.config.token_encryption_key,
        )
        .await?;
    }

    if let Some(enabled) = body.gitlab_enabled {
        auth_providers::set_bool(&mut conn, auth_providers::KEY_GITLAB_ENABLED, enabled).await?;
    }
    if let Some(ref base_url) = body.gitlab_base_url {
        let trimmed = base_url.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("http://") && !trimmed.starts_with("https://")
        {
            return Err(AppError::BadRequest(
                "gitlab_base_url must be an http/https URL".into(),
            ));
        }
        auth_providers::set_opt_setting(
            &mut conn,
            auth_providers::KEY_GITLAB_BASE_URL,
            Some(trimmed),
        )
        .await?;
    }
    if let Some(ref client_id) = body.gitlab_client_id {
        auth_providers::set_opt_setting(
            &mut conn,
            auth_providers::KEY_GITLAB_CLIENT_ID,
            Some(client_id.as_str()),
        )
        .await?;
    }
    if let Some(ref redirect_uri) = body.gitlab_redirect_uri {
        let trimmed = redirect_uri.trim();
        if !trimmed.is_empty() && !trimmed.starts_with("http://") && !trimmed.starts_with("https://")
        {
            return Err(AppError::BadRequest(
                "gitlab_redirect_uri must be an http/https URL".into(),
            ));
        }
        auth_providers::set_opt_setting(
            &mut conn,
            auth_providers::KEY_GITLAB_REDIRECT_URI,
            Some(trimmed),
        )
        .await?;
    }
    if body.gitlab_client_secret.is_some() {
        auth_providers::set_secret(
            &mut conn,
            auth_providers::KEY_GITLAB_CLIENT_SECRET,
            body.gitlab_client_secret.as_deref(),
            &state.config.token_encryption_key,
        )
        .await?;
    }

    let meta = serde_json::json!({
        "local_enabled": body.local_enabled,
        "github_enabled": body.github_enabled,
        "github_client_id": body.github_client_id.as_ref().map(|_| true),
        "github_redirect_uri": body.github_redirect_uri,
        "gitlab_enabled": body.gitlab_enabled,
        "gitlab_base_url": body.gitlab_base_url,
        "gitlab_client_id": body.gitlab_client_id.as_ref().map(|_| true),
        "gitlab_redirect_uri": body.gitlab_redirect_uri,
    });
    audit(
        &mut conn,
        admin_id,
        "update_auth_providers",
        "settings",
        None,
        Some(meta),
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct OAuthLoginQuery {
    pub return_to: Option<String>,
}

pub async fn github_login(
    State(state): State<AppState>,
    Query(q): Query<OAuthLoginQuery>,
) -> Result<Redirect, AppError> {
    oauth_login(&state, "github", q.return_to.as_deref()).await
}

pub async fn gitlab_login(
    State(state): State<AppState>,
    Query(q): Query<OAuthLoginQuery>,
) -> Result<Redirect, AppError> {
    oauth_login(&state, "gitlab", q.return_to.as_deref()).await
}

async fn oauth_login(
    state: &AppState,
    provider: &str,
    return_to: Option<&str>,
) -> Result<Redirect, AppError> {
    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
    let return_to = safe_return_to(return_to);
    let state_token = encode_oauth_state(provider, return_to, &state.config.oauth_state_secret)?;

    match provider {
        "github" => {
            let enabled = auth_providers::bool_setting(
                &mut conn,
                auth_providers::KEY_GITHUB_ENABLED,
                false,
            )
            .await;
            if !enabled {
                return Err(AppError::Forbidden("GitHub sign-in is disabled".into()));
            }
            let client_id = auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITHUB_CLIENT_ID)
                .await
                .ok_or_else(|| AppError::Forbidden("GitHub sign-in is not configured".into()))?;
            let redirect_uri = auth_providers::opt_setting(
                &mut conn,
                auth_providers::KEY_GITHUB_REDIRECT_URI,
            )
            .await
            .ok_or_else(|| AppError::Forbidden("GitHub sign-in is not configured".into()))?;
            let url = format!(
                "https://github.com/login/oauth/authorize?client_id={client_id}&redirect_uri={redirect_uri}&scope=read%3Auser%20user%3Aemail&state={state_token}"
            );
            Ok(Redirect::to(&url))
        }
        "gitlab" => {
            let enabled = auth_providers::bool_setting(
                &mut conn,
                auth_providers::KEY_GITLAB_ENABLED,
                false,
            )
            .await;
            if !enabled {
                return Err(AppError::Forbidden("GitLab sign-in is disabled".into()));
            }
            let base_url = auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_BASE_URL)
                .await
                .ok_or_else(|| AppError::Forbidden("GitLab sign-in is not configured".into()))?;
            let client_id = auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_CLIENT_ID)
                .await
                .ok_or_else(|| AppError::Forbidden("GitLab sign-in is not configured".into()))?;
            let redirect_uri = auth_providers::opt_setting(
                &mut conn,
                auth_providers::KEY_GITLAB_REDIRECT_URI,
            )
            .await
            .ok_or_else(|| AppError::Forbidden("GitLab sign-in is not configured".into()))?;
            let url = format!(
                "{base_url}/oauth/authorize?client_id={client_id}&redirect_uri={redirect_uri}&response_type=code&scope=read_user%20email&state={state_token}"
            );
            Ok(Redirect::to(&url))
        }
        _ => Err(AppError::BadRequest("Unsupported provider".into())),
    }
}

#[derive(Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

pub async fn github_callback(
    State(state): State<AppState>,
    Query(q): Query<OAuthCallbackQuery>,
) -> Redirect {
    oauth_callback(&state, "github", q).await
}

pub async fn gitlab_callback(
    State(state): State<AppState>,
    Query(q): Query<OAuthCallbackQuery>,
) -> Redirect {
    oauth_callback(&state, "gitlab", q).await
}

async fn oauth_callback(
    state: &AppState,
    provider: &str,
    q: OAuthCallbackQuery,
) -> Redirect {
    let redirect_err = || {
        Redirect::to(&format!(
            "{}auth/callback?error=oauth",
            state.config.frontend_origin.trim_end_matches('/')
        ))
    };

    if q.error.is_some() {
        return redirect_err();
    }

    let (code, raw_state) = match (q.code, q.state) {
        (Some(c), Some(s)) => (c, s),
        _ => return redirect_err(),
    };

    let oauth_state = match decode_oauth_state(&raw_state, &state.config.oauth_state_secret) {
        Ok(s) => s,
        Err(_) => return redirect_err(),
    };
    if oauth_state.provider != provider {
        return redirect_err();
    }

    let mut conn = match state.db.get().await {
        Ok(c) => c,
        Err(_) => return redirect_err(),
    };

    let (provider_account_id, email, display_name) = match provider_profile(state, provider, &code)
        .await
    {
        Ok(v) => v,
        Err(_) => return redirect_err(),
    };

    let random_password_hash = match hash_password(&Uuid::new_v4().to_string()) {
        Ok(v) => v,
        Err(_) => return redirect_err(),
    };

    let user: User = match diesel::sql_query("SELECT * FROM auth.upsert_oauth_user($1, $2, $3, $4, $5)")
        .bind::<diesel::sql_types::Text, _>(provider)
        .bind::<diesel::sql_types::Text, _>(&provider_account_id)
        .bind::<diesel::sql_types::Text, _>(email.as_deref().unwrap_or(""))
        .bind::<diesel::sql_types::Text, _>(display_name.as_deref().unwrap_or(""))
        .bind::<diesel::sql_types::Text, _>(&random_password_hash)
        .get_result(&mut conn)
        .await
    {
        Ok(user) => user,
        Err(_) => return redirect_err(),
    };

    let claims = Claims::new(
        &user.id.to_string(),
        &user.username,
        user.is_admin,
        state.config.jwt_expiry_hours,
    );
    let token = match encode_jwt(&claims, &state.config.jwt_secret) {
        Ok(token) => token,
        Err(_) => return redirect_err(),
    };

    let _ = audit(
        &mut conn,
        Some(user.id),
        "oauth_login",
        "user",
        Some(user.id),
        Some(serde_json::json!({ "provider": provider })),
    )
    .await;

    Redirect::to(&format!(
        "{}/auth/callback?token={}&return_to={}",
        state.config.frontend_origin.trim_end_matches('/'),
        token,
        oauth_state.return_to
    ))
}

#[derive(Deserialize)]
struct GithubTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GithubUser {
    id: i64,
    login: String,
    email: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct GitlabTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GitlabUser {
    id: i64,
    username: String,
    email: Option<String>,
    name: Option<String>,
}

async fn provider_profile(
    state: &AppState,
    provider: &str,
    code: &str,
) -> Result<(String, Option<String>, Option<String>), AppError> {
    match provider {
        "github" => {
            let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
            let client_id =
                auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITHUB_CLIENT_ID)
                    .await
                    .ok_or_else(|| {
                        AppError::Forbidden("GitHub sign-in is not configured".into())
                    })?;
            let client_secret = auth_providers::get_secret(
                &mut conn,
                auth_providers::KEY_GITHUB_CLIENT_SECRET,
                &state.config.token_encryption_key,
            )
            .await?
            .ok_or_else(|| AppError::Forbidden("GitHub sign-in is not configured".into()))?;
            let redirect_uri =
                auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITHUB_REDIRECT_URI)
                    .await
                    .ok_or_else(|| {
                        AppError::Forbidden("GitHub sign-in is not configured".into())
                    })?;
            let token = github_exchange_code(
                &state.http_client,
                &client_id,
                &client_secret,
                &redirect_uri,
                code,
            )
            .await?;
            let user = github_get_user(&state.http_client, &token).await?;
            Ok((
                user.id.to_string(),
                user.email,
                Some(user.name.unwrap_or(user.login)),
            ))
        }
        "gitlab" => {
            let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
            let base_url =
                auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_BASE_URL)
                    .await
                    .ok_or_else(|| AppError::Forbidden("GitLab sign-in is not configured".into()))?;
            let client_id =
                auth_providers::opt_setting(&mut conn, auth_providers::KEY_GITLAB_CLIENT_ID)
                    .await
                    .ok_or_else(|| AppError::Forbidden("GitLab sign-in is not configured".into()))?;
            let client_secret = auth_providers::get_secret(
                &mut conn,
                auth_providers::KEY_GITLAB_CLIENT_SECRET,
                &state.config.token_encryption_key,
            )
            .await?
            .ok_or_else(|| AppError::Forbidden("GitLab sign-in is not configured".into()))?;
            let redirect_uri = auth_providers::opt_setting(
                &mut conn,
                auth_providers::KEY_GITLAB_REDIRECT_URI,
            )
            .await
            .ok_or_else(|| AppError::Forbidden("GitLab sign-in is not configured".into()))?;
            let token = gitlab_exchange_code(
                &state.http_client,
                &base_url,
                &client_id,
                &client_secret,
                &redirect_uri,
                code,
            )
            .await?;
            let user = gitlab_get_user(&state.http_client, &base_url, &token).await?;
            Ok((
                user.id.to_string(),
                user.email,
                Some(user.name.unwrap_or(user.username)),
            ))
        }
        _ => Err(AppError::BadRequest("Unsupported provider".into())),
    }
}

async fn github_exchange_code(
    client: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> Result<String, AppError> {
    #[derive(Serialize)]
    struct Body<'a> {
        client_id: &'a str,
        client_secret: &'a str,
        code: &'a str,
        redirect_uri: &'a str,
    }

    let resp = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "helm-hub/1.0")
        .json(&Body {
            client_id,
            client_secret,
            code,
            redirect_uri,
        })
        .send()
        .await
        .map_err(|_| AppError::Internal("GitHub token exchange failed".into()))?;

    let token_resp: GithubTokenResponse = resp
        .json()
        .await
        .map_err(|_| AppError::Internal("Failed to parse GitHub token response".into()))?;
    Ok(token_resp.access_token)
}

async fn github_get_user(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<GithubUser, AppError> {
    let resp = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "helm-hub/1.0")
        .send()
        .await
        .map_err(|_| AppError::Internal("GitHub user fetch failed".into()))?;

    resp.json::<GithubUser>()
        .await
        .map_err(|_| AppError::Internal("Failed to parse GitHub user response".into()))
}

async fn gitlab_exchange_code(
    client: &reqwest::Client,
    base_url: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> Result<String, AppError> {
    #[derive(Serialize)]
    struct Body<'a> {
        client_id: &'a str,
        client_secret: &'a str,
        code: &'a str,
        grant_type: &'a str,
        redirect_uri: &'a str,
    }

    let resp = client
        .post(format!("{base_url}/oauth/token"))
        .header("User-Agent", "helm-hub/1.0")
        .json(&Body {
            client_id,
            client_secret,
            code,
            grant_type: "authorization_code",
            redirect_uri,
        })
        .send()
        .await
        .map_err(|_| AppError::Internal("GitLab token exchange failed".into()))?;

    let token_resp: GitlabTokenResponse = resp
        .json()
        .await
        .map_err(|_| AppError::Internal("Failed to parse GitLab token response".into()))?;
    Ok(token_resp.access_token)
}

async fn gitlab_get_user(
    client: &reqwest::Client,
    base_url: &str,
    access_token: &str,
) -> Result<GitlabUser, AppError> {
    let resp = client
        .get(format!("{base_url}/api/v4/user"))
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", "helm-hub/1.0")
        .send()
        .await
        .map_err(|_| AppError::Internal("GitLab user fetch failed".into()))?;

    resp.json::<GitlabUser>()
        .await
        .map_err(|_| AppError::Internal("Failed to parse GitLab user response".into()))
}

pub async fn me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<User>, AppError> {
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|e| AppError::Internal(format!("Invalid user_id in claims: {e}")))?;
    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
    let user: User = users::table
        .filter(users::id.eq(user_id))
        .select(User::as_select())
        .first(&mut conn)
        .await?;
    Ok(Json(user))
}
