use base32::{encode, Alphabet};
use rand::RngCore;
use totp_rs::{Algorithm, Secret, TOTP};

use crate::error::AppError;

/// Generate a new random base-32 TOTP secret.
pub fn generate_secret() -> String {
    let mut bytes = [0u8; 20]; // 160-bit secret
    rand::thread_rng().fill_bytes(&mut bytes);
    encode(Alphabet::RFC4648 { padding: false }, &bytes)
}

/// Build the `otpauth://` provisioning URI shown in QR codes.
pub fn provisioning_uri(username: &str, secret_b32: &str, issuer: &str) -> Result<String, AppError> {
    let totp = build_totp(username, secret_b32, issuer)?;
    Ok(totp.get_url())
}

/// Verify a 6-digit TOTP code against the stored secret.
pub fn verify_code(secret_b32: &str, code: &str, username: &str, issuer: &str) -> Result<bool, AppError> {
    let totp = build_totp(username, secret_b32, issuer)?;
    totp.check_current(code).map_err(|e| AppError::Internal(e.to_string()))
}

fn build_totp(username: &str, secret_b32: &str, issuer: &str) -> Result<TOTP, AppError> {
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        Secret::Encoded(secret_b32.to_string())
            .to_bytes()
            .map_err(|e| AppError::Internal(e.to_string()))?,
        Some(issuer.to_string()),
        username.to_string(),
    )
    .map_err(|e| AppError::Internal(e.to_string()))
}
