use axum::{Json, extract::State};
use chrono::{DateTime, Timelike, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    AppState,
    auth::{
        jwt::{Claims, encode_jwt},
        password::{hash_password, verify_password},
        totp::{generate_secret, provisioning_uri, verify_code},
    },
    db::models::{NewUser, UpdateUser, User},
    error::AppError,
    schema::{rate_limit_windows, users},
    services::{audit::audit, settings},
};

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

    let Ok(mut conn) = state.db.get().await else { return };

    let _ = conn.transaction::<(), AppError, _>(async |conn| {
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
    }).await;
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
        let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
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
    let new_user = NewUser::new(req.username, req.email, hash);

    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
    diesel::insert_into(users::table)
        .values(&new_user)
        .execute(&mut conn)
        .await?;

    let user: User = users::table
        .filter(users::id.eq(&new_user.id))
        .select(User::as_select())
        .first(&mut conn)
        .await?;

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
    pub user: User,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AppError> {
    // Check failed-login counter before touching the DB for the user record.
    check_failed_logins(&state, &req.username).await?;

    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;

    // Use a constant-time-friendly error: same message for "no such user" and
    // "wrong password" to prevent username enumeration.
    let mut user: User = users::table
        .filter(users::username.eq(&req.username))
        .select(User::as_select())
        .first(&mut conn)
        .await
        .map_err(|_| {
            let state_clone = state.clone();
            let username_clone = req.username.clone();
            tokio::spawn(async move {
                record_failed_login(&state_clone, &username_clone).await;
            });
            AppError::Unauthorized("Invalid credentials".into())
        })?;

    if !verify_password(&req.password, &user.password_hash)? {
        record_failed_login(&state, &req.username).await;
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }

    if user.is_banned() {
        return Err(AppError::Forbidden("Account suspended".into()));
    }

    // Auto-promote the bootstrap admin on their first login so the DB stays
    // consistent and the returned user object / JWT both carry is_admin=true.
    if !user.is_admin && state.config.admin_username.as_deref() == Some(user.username.as_str()) {
        let _ = diesel::update(users::table.filter(users::id.eq(&user.id)))
            .set((
                crate::schema::users::is_admin.eq(true),
                crate::schema::users::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .await;
        user.is_admin = true;
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

    let claims = Claims::new(
        &user.id.to_string(),
        &user.username,
        user.is_admin,
        state.config.jwt_expiry_hours,
    );
    let token = encode_jwt(&claims, &state.config.jwt_secret)?;

    audit(
        &mut conn,
        Some(user.id),
        "login",
        "user",
        Some(user.id),
        None,
    )
    .await?;

    Ok(Json(LoginResponse { token, user }))
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

    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
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
    let mut conn = state.db.get().await.map_err(|e| AppError::Pool(e.to_string()))?;
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
        assert!(validate_email(&( "a".repeat(255) + "@b.c")).is_err());
    }
}
