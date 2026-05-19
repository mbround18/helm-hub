use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::{error::AppError, db::user_role::UserRole};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub is_admin: bool,
    #[serde(default)]
    pub role: Option<String>,
    pub iat: i64,
    pub exp: i64,
}

impl Claims {
    pub fn new(user_id: &str, username: &str, is_admin: bool, role: UserRole, expiry_hours: i64) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id.to_string(),
            username: username.to_string(),
            is_admin,
            role: Some(role.as_str().to_string()),
            iat: now.timestamp(),
            exp: (now + Duration::hours(expiry_hours)).timestamp(),
        }
    }

    pub fn new_with_minutes(user_id: &str, username: &str, is_admin: bool, role: UserRole, expiry_minutes: i64) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id.to_string(),
            username: username.to_string(),
            is_admin,
            role: Some(role.as_str().to_string()),
            iat: now.timestamp(),
            exp: (now + Duration::minutes(expiry_minutes)).timestamp(),
        }
    }

    pub fn user_role(&self) -> Result<UserRole, AppError> {
        if let Some(role_str) = self.role.as_deref() {
            return UserRole::parse(role_str).ok_or_else(|| {
                AppError::Unauthorized(format!("Invalid role in token: {role_str}"))
            });
        }

        // Backward compatibility for legacy tokens that predate RBAC claims.
        if self.is_admin {
            Ok(UserRole::Admin)
        } else {
            Ok(UserRole::User)
        }
    }
}

pub fn encode_jwt(claims: &Claims, secret: &str) -> Result<String, AppError> {
    encode(
        &Header::new(Algorithm::HS256),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.to_string()))
}

pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, AppError> {
    // Explicitly pin to HS256 — rejects "none" and any other algorithm.
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Unauthorized(e.to_string()))
}

/// Decode JWT without checking expiry (for refresh token endpoint).
/// Allows decoding expired tokens, but still validates signature.
pub fn decode_jwt_without_expiry_check(token: &str, secret: &str) -> Result<Claims, AppError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.leeway = 0;
    validation.validate_exp = false; // Skip expiry check
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Unauthorized(e.to_string()))
}
