//! AES-256-GCM encryption for GitHub access tokens stored at rest.
//!
//! Each token is stored as `{12-byte nonce hex}:{ciphertext hex}`.
//! A fresh random nonce is generated per encryption so repeated encryptions
//! of the same plaintext produce different ciphertexts.

use aes_gcm::{
    AeadCore, Aes256Gcm, KeyInit,
    aead::{Aead, OsRng},
};

use crate::error::AppError;

pub fn encrypt_token(plaintext: &str, key: &[u8; 32]) -> Result<String, AppError> {
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| AppError::Internal(format!("Token encryption failed: {e}")))?;
    Ok(format!(
        "{}:{}",
        hex::encode(nonce),
        hex::encode(ciphertext)
    ))
}

pub fn decrypt_token(stored: &str, key: &[u8; 32]) -> Result<String, AppError> {
    let (nonce_hex, ct_hex) = stored
        .split_once(':')
        .ok_or_else(|| AppError::Internal("Malformed encrypted token".into()))?;

    let nonce_bytes =
        hex::decode(nonce_hex).map_err(|_| AppError::Internal("Invalid token nonce".into()))?;
    let nonce = aes_gcm::Nonce::from_slice(&nonce_bytes);

    let ct =
        hex::decode(ct_hex).map_err(|_| AppError::Internal("Invalid token ciphertext".into()))?;

    let cipher = Aes256Gcm::new(key.into());
    let plain = cipher
        .decrypt(nonce, ct.as_ref())
        .map_err(|_| AppError::Internal("Token decryption failed".into()))?;

    String::from_utf8(plain).map_err(|_| AppError::Internal("Token UTF-8 error".into()))
}
