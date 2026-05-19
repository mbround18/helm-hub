use axum::{Json, extract::State};
use chrono::{DateTime, Timelike, Utc};
use diesel::prelude::*;
use diesel::sql_query;
use diesel_async::{AsyncConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{
        jwt::{Claims, decode_jwt_without_expiry_check, encode_jwt},
        password::{hash_password, verify_password},
        totp::{generate_secret, provisioning_uri, verify_code},
    },
    db::{
        DbConn,
        models::{UpdateUser, User},
        user_role::UserRole,
    },
    error::AppError,
    schema::{rate_limit_windows, users},
    services::{audit::audit, settings},
};

// ── SQL Query Result Structs ──────────────────────────────────────────────────
use diesel::deserialize::QueryableByName;

#[derive(QueryableByName, Debug)]
struct RefreshTokenStruct {
    #[diesel(sql_type = diesel::sql_types::Text)]
    token: String,
    #[diesel(sql_type = diesel::sql_types::Timestamptz)]
    expires_at: DateTime<Utc>,
}

#[derive(QueryableByName, Debug)]
struct RefreshProcResult {
    #[diesel(sql_type = diesel::sql_types::Bool)]
    success: bool,
    #[diesel(sql_type = diesel::sql_types::Text)]
    message: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    new_token: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    new_refresh_token: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Timestamptz>)]
    expires_at: Option<DateTime<Utc>>,
}

// ── Input validation ──────────────────────────────────────────────────────────

fn validate_username(username: &str) -> Result<(), AppError> {
    if username.len() < 2 || username.len() > 39 {
        return Err(AppError::BadRequest(
            "Username must be 2–39 characters".into(),
        ));
    }
    // GitHub-style: alphanumeric + single hyphens, no leading/trailing hyphen,
    // no consecutive hyphens.  This also ensures the username is safe as a
    // filesystem path segment (no traversal sequences possible).
    let valid = username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
        && !username.starts_with('-')
        && !username.ends_with('-')
        && !username.contains("--");
    if !valid {
        return Err(AppError::BadRequest(
            "Username may only contain alphanumeric characters and single \
             hyphens, and cannot start or end with a hyphen"
                .into(),
        ));
    }
    Ok(())
}

fn validate_email(email: &str) -> Result<(), AppError> {
    if email.len() > 254 || !email.contains('@') || email.starts_with('@') {
        return Err(AppError::BadRequest("Invalid email address".into()));
    }
    Ok(())
}

fn is_bootstrap_owner(admin_username: Option<&str>, username: &str) -> bool {
    admin_username
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.eq_ignore_ascii_case(username))
        .unwrap_or(false)
}

pub(crate) async fn seed_user_auth_state(
    conn: &mut DbConn,
    user: &mut User,
    admin_username: Option<&str>,
) -> Result<(), AppError> {
    let mut target_role = user.role;
    let mut target_is_admin = user.is_admin;
    let mut target_totp_enabled = user.totp_enabled;
    let mut target_storage_usage = user.storage_usage_bytes;

    if is_bootstrap_owner(admin_username, &user.username) {
        target_role = UserRole::Owner;
        target_is_admin = true;
    } else {
        if user.role == UserRole::User && user.is_admin {
            target_role = UserRole::Admin;
        }
        if user.role.is_admin_or_higher() && !user.is_admin {
            target_is_admin = true;
        }
    }

    if user.totp_enabled && user.totp_secret.is_none() {
        target_totp_enabled = false;
    }

    if user.storage_usage_bytes < 0 {
        target_storage_usage = 0;
    }

    let needs_update = target_role != user.role
        || target_is_admin != user.is_admin
        || target_totp_enabled != user.totp_enabled
        || target_storage_usage != user.storage_usage_bytes;

    if !needs_update {
        return Ok(());
    }

    sql_query("SELECT set_config('app.auth_context', 'true', true)")
        .execute(conn)
        .await?;

    let updated = sql_query(
        "UPDATE users
         SET role = $1::user_role,
             is_admin = $2,
             totp_enabled = $3,
             storage_usage_bytes = $4,
             updated_at = $5
         WHERE id = $6",
    )
    .bind::<diesel::sql_types::Text, _>(target_role.as_str())
    .bind::<diesel::sql_types::Bool, _>(target_is_admin)
    .bind::<diesel::sql_types::Bool, _>(target_totp_enabled)
    .bind::<diesel::sql_types::BigInt, _>(target_storage_usage)
    .bind::<diesel::sql_types::Timestamptz, _>(Utc::now())
    .bind::<diesel::sql_types::Uuid, _>(user.id)
    .execute(conn)
    .await?;

    if updated == 0 {
        return Err(AppError::NotFound(
            "User not found during auth state seeding".into(),
        ));
    }

    user.role = target_role;
    user.is_admin = target_is_admin;
    user.totp_enabled = target_totp_enabled;
    user.storage_usage_bytes = target_storage_usage;
    Ok(())
}

// ── Brute-force protection ────────────────────────────────────────────────────

/// 15-minute window key for failed login tracking.
fn login_window() -> DateTime<Utc> {
    let now = Utc::now();
    // Rounded down to the nearest 15-minute boundary.
    let quarter = now.minute() / 15;
    now.with_minute(quarter * 15)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap()
}

/// Returns the rate-limit key for failed login attempts for a given username.
/// We hash the username so the DB row doesn't store a plaintext username.
fn login_attempt_key(username: &str) -> String {
    let hash = Sha256::digest(username.as_bytes());
    format!(
        "login:{}",
        hash.iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

/// Maximum failed login attempts per 15-minute window before the account is
/// temporarily locked.
const MAX_FAILED_LOGINS: i32 = 10;

/// Returns `Err(TooManyRequests)` if the caller has exceeded the failed-login
/// limit.  Does NOT increment the counter — call `record_failed_login` on
/// failure.  Fails **open** on DB errors so a storage hiccup never locks
/// everyone out.
async fn check_failed_logins(state: &AppState, username: &str) -> Result<(), AppError> {
    let key = login_attempt_key(username);
    let window = login_window();

    let mut conn = match state.db.get().await {
        Ok(c) => c,
        Err(_) => return Ok(()), // fail open
    };

    let existing: Option<(i32, DateTime<Utc>)> = rate_limit_windows::table
        .find(&key)
        .select((rate_limit_windows::count, rate_limit_windows::window_start))
        .first(&mut conn)
        .await
        .optional()
        .unwrap_or(None);

    match existing {
        Some((count, ws)) if ws == window && count >= MAX_FAILED_LOGINS => {
            Err(AppError::TooManyRequests(
                "Too many failed login attempts. Please wait 15 minutes.".into(),
            ))
        }
        _ => Ok(()),
    }
}

/// Increments the failed-login counter for `username`.  Fails silently on DB
/// errors so a storage issue never surfaces as a confusing error to the user.
async fn record_failed_login(state: &AppState, username: &str) {
    let key = login_attempt_key(username);
    let window = login_window();

    let Ok(mut conn) = state.db.get().await else {
        return;
    };

    let _ = conn
        .transaction::<(), AppError, _>(async |conn| {
            use rate_limit_windows::dsl::{
                count, key as key_col, rate_limit_windows as table, window_start,
            };

            let existing = table
                .find(&key)
                .select((count, window_start))
                .first::<(i32, DateTime<Utc>)>(conn)
                .await
                .optional()?;

            match existing {
                None => {
                    diesel::insert_into(table)
                        .values((key_col.eq(&key), count.eq(1), window_start.eq(&window)))
                        .execute(conn)
                        .await?;
                }
                Some((_, ws)) if ws < window => {
                    diesel::update(table.find(&key))
                        .set((count.eq(1), window_start.eq(&window)))
                        .execute(conn)
                        .await?;
                }
                Some((c, _)) => {
                    diesel::update(table.find(&key))
                        .set(count.eq(c + 1))
                        .execute(conn)
                        .await?;
                }
            }
            Ok(())
        })
        .await;
}

// ── Register ──────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user: User,
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, AppError> {
    {
        let mut conn = state
            .db
            .get()
            .await
            .map_err(|e| AppError::Pool(e.to_string()))?;
        if !settings::signup_enabled(&mut conn).await {
            return Err(AppError::Forbidden(
                "Registration is currently disabled".into(),
            ));
        }
    }
    validate_username(&req.username)?;
    validate_email(&req.email)?;
    if req.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".into(),
        ));
    }
    if req.password.len() > 1024 {
        return Err(AppError::BadRequest(
            "Password must be at most 1024 characters".into(),
        ));
    }

    let hash = hash_password(&req.password)?;
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;
    let admin_username = state.config.admin_username.as_deref().unwrap_or("");

    let mut user: User = sql_query("SELECT * FROM auth.register_user($1, $2, $3, $4)")
        .bind::<diesel::sql_types::Text, _>(&req.username)
        .bind::<diesel::sql_types::Text, _>(&req.email)
        .bind::<diesel::sql_types::Text, _>(&hash)
        .bind::<diesel::sql_types::Text, _>(admin_username)
        .get_result(&mut conn)
        .await?;

    if let Err(e) =
        seed_user_auth_state(&mut conn, &mut user, state.config.admin_username.as_deref()).await
    {
        tracing::warn!(user_id = %user.id, username = %user.username, error = %e, "Auth state seeding failed during register");
    }

    audit(
        &mut conn,
        Some(user.id),
        "register",
        "user",
        Some(user.id),
        None,
    )
    .await?;

    Ok(Json(RegisterResponse { user }))
}

// ── Login ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub totp_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub user: User,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    // Check failed-login counter before touching the DB for the user record.
    check_failed_logins(&state, &req.username).await?;
    let username = req.username.clone();

    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;
    let admin_username = state.config.admin_username.as_deref().unwrap_or("");

    // Use a constant-time-friendly error: same message for "no such user" and
    // "wrong password" to prevent username enumeration.
    let mut created_via_bootstrap = false;
    let mut user: User = match sql_query("SELECT * FROM auth.login_user($1, $2)")
        .bind::<diesel::sql_types::Text, _>(&username)
        .bind::<diesel::sql_types::Text, _>(admin_username)
        .get_result(&mut conn)
        .await
    {
        Ok(user) => user,
        Err(_) => {
            // Self-heal path: if bootstrap admin is missing, seed it on first login.
            if is_bootstrap_owner(state.config.admin_username.as_deref(), &username) {
                let hash = hash_password(&req.password)?;
                let bootstrap_email = format!("{username}@bootstrap.local");
                match sql_query("SELECT * FROM auth.register_user($1, $2, $3, $4)")
                    .bind::<diesel::sql_types::Text, _>(&username)
                    .bind::<diesel::sql_types::Text, _>(&bootstrap_email)
                    .bind::<diesel::sql_types::Text, _>(&hash)
                    .bind::<diesel::sql_types::Text, _>(admin_username)
                    .get_result::<User>(&mut conn)
                    .await
                {
                    Ok(user) => {
                        created_via_bootstrap = true;
                        tracing::info!(
                            username = %username,
                            user_id = %user.id,
                            "Bootstrapped missing admin user during login",
                        );
                        user
                    }
                    Err(_) => {
                        let state_clone = state.clone();
                        let username_clone = req.username.clone();
                        tokio::spawn(async move {
                            record_failed_login(&state_clone, &username_clone).await;
                        });
                        return Err(AppError::Unauthorized("Invalid credentials".into()));
                    }
                }
            } else {
                let state_clone = state.clone();
                let username_clone = req.username.clone();
                tokio::spawn(async move {
                    record_failed_login(&state_clone, &username_clone).await;
                });
                return Err(AppError::Unauthorized("Invalid credentials".into()));
            }
        }
    };

    if !created_via_bootstrap && !verify_password(&req.password, &user.password_hash)? {
        record_failed_login(&state, &req.username).await;
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }

    if user.is_banned() {
        return Err(AppError::Forbidden("Account suspended".into()));
    }

    if let Err(e) =
        seed_user_auth_state(&mut conn, &mut user, state.config.admin_username.as_deref()).await
    {
        tracing::warn!(user_id = %user.id, username = %user.username, error = %e, "Auth state seeding failed during login");
    }

    if user.totp_enabled {
        let code = req
            .totp_code
            .as_deref()
            .ok_or_else(|| AppError::Unauthorized("TOTP code required".into()))?;

        let secret = user
            .totp_secret
            .as_deref()
            .ok_or_else(|| AppError::Internal("TOTP enabled but secret missing".into()))?;

        if !verify_code(secret, code, &user.username, "HelmHub")? {
            record_failed_login(&state, &req.username).await;
            return Err(AppError::Unauthorized("Invalid TOTP code".into()));
        }
    }

    let claims = Claims::new_with_minutes(
        &user.id.to_string(),
        &user.username,
        user.is_admin,
        user.role,
        15, // 15 minutes
    );
    let token = encode_jwt(&claims, &state.config.jwt_secret)?;

    // Issue refresh token from stored procedure
    let refresh_results =
        sql_query("SELECT token, expires_at FROM auth.issue_refresh_token($1, 168)")
            .bind::<diesel::sql_types::Uuid, _>(user.id)
            .load::<RefreshTokenStruct>(&mut conn)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to issue refresh token: {}", e)))?;

    let refresh_result = refresh_results
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Internal("No refresh token returned".into()))?;

    let refresh_token_plaintext = refresh_result.token;
    let _refresh_expires_at = refresh_result.expires_at;

    // Encrypt refresh token for secure localStorage storage
    use crate::auth::token_encryption::encrypt_token;
    let refresh_token_encrypted = encrypt_token(&refresh_token_plaintext, &user.id)?;

    // Calculate JWT expiry in seconds for client
    let expires_in = 15 * 60; // 15 minutes in seconds

    audit(
        &mut conn,
        Some(user.id),
        "login",
        "user",
        Some(user.id),
        None,
    )
    .await?;

    Ok(Json(LoginResponse {
        token,
        refresh_token: refresh_token_encrypted,
        expires_in,
        user,
    }))
}

// ── Token Refresh ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub token: String,         // Old JWT (may be expired, but still decodable)
    pub refresh_token: String, // Encrypted refresh token
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<RefreshResponse>, AppError> {
    // Extract user_id from JWT claim before it expires
    // The user sends both the (possibly expired) JWT and refresh token
    let claims = decode_jwt_without_expiry_check(&req.token, &state.config.jwt_secret)?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|e| AppError::Internal(e.to_string()))?;

    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;

    // Decrypt the refresh token
    use crate::auth::token_encryption::decrypt_token;
    let refresh_token_plaintext = decrypt_token(&req.refresh_token, &user_id)?;

    // Hash refresh token for DB lookup (same as stored in DB)
    let token_hash = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(refresh_token_plaintext.as_bytes()))
    );

    // Call stored procedure to validate and rotate token
    let results = sql_query(
        "SELECT success, message, new_token, new_refresh_token, expires_at FROM auth.refresh_access_token($1, $2)"
    )
        .bind::<diesel::sql_types::Text, _>(&token_hash)
        .bind::<diesel::sql_types::Uuid, _>(user_id)
        .load::<RefreshProcResult>(&mut conn)
        .await
        .map_err(|e| AppError::Internal(format!("Token refresh failed: {}", e)))?;

    let result = results
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Internal("No result from refresh token procedure".into()))?;

    if !result.success {
        return Err(AppError::Unauthorized(result.message));
    }

    // Get the new refresh token plaintext from stored procedure (will issue new one)
    let new_refresh_token_plaintext = result.new_refresh_token.ok_or_else(|| {
        AppError::Internal("Stored procedure did not return new refresh token".into())
    })?;

    // Encrypt the new refresh token
    use crate::auth::token_encryption::encrypt_token;
    let new_refresh_token_encrypted = encrypt_token(&new_refresh_token_plaintext, &user_id)?;

    // Get user for new JWT
    let user: User = users::table
        .find(user_id)
        .first(&mut conn)
        .await
        .map_err(|_| AppError::Unauthorized("User not found".into()))?;

    // Issue new JWT (short-lived, 15 minutes)
    let new_claims = Claims::new_with_minutes(
        &user.id.to_string(),
        &user.username,
        user.is_admin,
        user.role,
        15, // 15 minutes
    );
    let new_token = encode_jwt(&new_claims, &state.config.jwt_secret)?;

    Ok(Json(RefreshResponse {
        token: new_token,
        refresh_token: new_refresh_token_encrypted,
        expires_in: 15 * 60, // 15 minutes in seconds
    }))
}

// ── TOTP Setup ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct TotpSetupResponse {
    pub secret: String,
    pub provisioning_uri: String,
}

pub async fn totp_setup(
    State(state): State<AppState>,
    claims: axum::extract::Extension<Claims>,
) -> Result<Json<TotpSetupResponse>, AppError> {
    let secret = generate_secret();
    let uri = provisioning_uri(&claims.username, &secret, "HelmHub")?;

    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|e| AppError::Internal(e.to_string()))?;
    diesel::update(users::table.filter(users::id.eq(&user_id)))
        .set(&UpdateUser {
            totp_secret: Some(Some(secret.clone())),
            ..Default::default()
        })
        .execute(&mut conn)
        .await?;

    Ok(Json(TotpSetupResponse {
        secret,
        provisioning_uri: uri,
    }))
}

#[derive(Debug, Deserialize)]
pub struct TotpVerifyRequest {
    pub code: String,
}

pub async fn totp_enable(
    State(state): State<AppState>,
    claims: axum::extract::Extension<Claims>,
    Json(req): Json<TotpVerifyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut conn = state
        .db
        .get()
        .await
        .map_err(|e| AppError::Pool(e.to_string()))?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|e| AppError::Internal(e.to_string()))?;

    let user: User = users::table
        .filter(users::id.eq(&user_id))
        .select(User::as_select())
        .first(&mut conn)
        .await?;

    let secret = user
        .totp_secret
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Run TOTP setup first".into()))?;

    if !verify_code(secret, &req.code, &user.username, "HelmHub")? {
        return Err(AppError::BadRequest("Invalid TOTP code".into()));
    }

    diesel::update(users::table.filter(users::id.eq(&user_id)))
        .set(&UpdateUser {
            totp_enabled: Some(true),
            ..Default::default()
        })
        .execute(&mut conn)
        .await?;

    let _ = audit(
        &mut conn,
        Some(user_id),
        "enable_totp",
        "user",
        Some(user_id),
        None,
    )
    .await;

    Ok(Json(
        serde_json::json!({ "message": "TOTP enabled successfully" }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_username() {
        // Valid
        assert!(validate_username("valid-user").is_ok());
        assert!(validate_username("user123").is_ok());
        assert!(validate_username("a-b").is_ok());
        assert!(validate_username("a").is_err()); // Too short

        // Invalid: Length
        assert!(validate_username("a").is_err());
        assert!(validate_username(&"a".repeat(40)).is_err());

        // Invalid: Characters
        assert!(validate_username("user!name").is_err());
        assert!(validate_username("user_name").is_err());
        assert!(validate_username("user name").is_err());

        // Invalid: Hyphens
        assert!(validate_username("-user").is_err());
        assert!(validate_username("user-").is_err());
        assert!(validate_username("user--name").is_err());
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("u@e.c").is_ok());

        assert!(validate_email("no-at-sign").is_err());
        assert!(validate_email("@starts-with-at.com").is_err());
        assert!(validate_email(&("a".repeat(255) + "@b.c")).is_err());
    }
}
