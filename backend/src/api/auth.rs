use axum::{extract::State, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    auth::{
        jwt::{encode_jwt, Claims},
        password::{hash_password, verify_password},
        totp::{generate_secret, provisioning_uri, verify_code},
    },
    db::models::{NewUser, UpdateUser, User},
    error::AppError,
    schema::users,
    AppState,
};

// ── Register ─────────────────────────────────────────────────────────────────

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
    if req.password.len() < 8 {
        return Err(AppError::BadRequest("Password must be at least 8 characters".into()));
    }

    let hash = hash_password(&req.password)?;
    let new_user = NewUser::new(req.username, req.email, hash);

    let mut conn = state.db.get()?;
    diesel::insert_into(users::table)
        .values(&new_user)
        .execute(&mut conn)?;

    let user: User = users::table
        .filter(users::id.eq(&new_user.id))
        .select(User::as_select())
        .first(&mut conn)?;

    Ok(Json(RegisterResponse { user }))
}

// ── Login ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    /// Required only when TOTP is enabled on the account.
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
    let mut conn = state.db.get()?;

    let user: User = users::table
        .filter(users::username.eq(&req.username))
        .select(User::as_select())
        .first(&mut conn)
        .map_err(|_| AppError::Unauthorized("Invalid credentials".into()))?;

    if !verify_password(&req.password, &user.password_hash)? {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }

    if user.is_totp_enabled() {
        let code = req
            .totp_code
            .as_deref()
            .ok_or_else(|| AppError::Unauthorized("TOTP code required".into()))?;

        let secret = user
            .totp_secret
            .as_deref()
            .ok_or_else(|| AppError::Internal("TOTP enabled but secret missing".into()))?;

        if !verify_code(secret, code, &user.username, "HelmHub")? {
            return Err(AppError::Unauthorized("Invalid TOTP code".into()));
        }
    }

    let claims = Claims::new(&user.id, &user.username, user.is_admin(), state.config.jwt_expiry_hours);
    let token = encode_jwt(&claims, &state.config.jwt_secret)?;

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

    let mut conn = state.db.get()?;
    diesel::update(users::table.filter(users::id.eq(&claims.sub)))
        .set(&UpdateUser {
            totp_secret: Some(Some(secret.clone())),
            ..Default::default()
        })
        .execute(&mut conn)?;

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
    let mut conn = state.db.get()?;

    let user: User = users::table
        .filter(users::id.eq(&claims.sub))
        .select(User::as_select())
        .first(&mut conn)?;

    let secret = user
        .totp_secret
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("Run TOTP setup first".into()))?;

    if !verify_code(secret, &req.code, &user.username, "HelmHub")? {
        return Err(AppError::BadRequest("Invalid TOTP code".into()));
    }

    diesel::update(users::table.filter(users::id.eq(&claims.sub)))
        .set(&UpdateUser {
            totp_enabled: Some(1),
            ..Default::default()
        })
        .execute(&mut conn)?;

    Ok(Json(serde_json::json!({ "message": "TOTP enabled successfully" })))
}
