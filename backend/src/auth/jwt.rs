use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub is_admin: bool,
    pub iat: i64,
    pub exp: i64,
}

impl Claims {
    pub fn new(user_id: &str, username: &str, is_admin: bool, expiry_hours: i64) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id.to_string(),
            username: username.to_string(),
            is_admin,
            iat: now.timestamp(),
            exp: (now + Duration::hours(expiry_hours)).timestamp(),
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
    decode::<Claims>(token, &DecodingKey::from_secret(secret.as_bytes()), &validation)
        .map(|data| data.claims)
        .map_err(|e| AppError::Unauthorized(e.to_string()))
}
