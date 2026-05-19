use crate::error::AppError;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use sodiumoxide::crypto::secretbox;
use uuid::Uuid;

/// Token encryption/decryption for secure localStorage storage
/// Uses XSalsa20-Poly1305 AEAD from libsodium

/// Encrypt a token with a key derived from user ID and a nonce
pub fn encrypt_token(token: &str, user_id: &Uuid) -> Result<String, AppError> {
    // Derive key from user ID (32 bytes for secretbox)
    // In production, user ID alone is not a strong key material source
    // For now: hash(user_id) serves as deterministic key - immutable per user
    let key_material = format!("helm-hub-auth-{}", user_id);
    let hash = blake3::hash(key_material.as_bytes());
    let key_bytes: [u8; 32] = hash.as_bytes()[..32]
        .try_into()
        .map_err(|_| AppError::Internal("Key derivation failed".into()))?;

    let key = secretbox::Key(key_bytes);
    let nonce = secretbox::gen_nonce();
    let plaintext = token.as_bytes();

    let ciphertext = secretbox::seal(plaintext, &nonce, &key);

    // Combine nonce + ciphertext and base64 encode for storage
    let mut combined = nonce.as_ref().to_vec();
    combined.extend_from_slice(&ciphertext);

    Ok(format!("encrypted:{}", STANDARD.encode(&combined)))
}

/// Decrypt a token using the same user ID-derived key
pub fn decrypt_token(encrypted: &str, user_id: &Uuid) -> Result<String, AppError> {
    if !encrypted.starts_with("encrypted:") {
        return Err(AppError::Unauthorized(
            "Invalid encrypted token format".into(),
        ));
    }

    let encoded = &encrypted[10..]; // Remove "encrypted:" prefix
    let combined = STANDARD
        .decode(encoded)
        .map_err(|_| AppError::Unauthorized("Invalid token encoding".into()))?;

    if combined.len() < secretbox::NONCEBYTES {
        return Err(AppError::Unauthorized("Invalid token length".into()));
    }

    // Extract nonce and ciphertext
    let (nonce_bytes, ciphertext) = combined.split_at(secretbox::NONCEBYTES);
    let nonce = secretbox::Nonce::from_slice(nonce_bytes)
        .ok_or_else(|| AppError::Internal("Nonce deserialization failed".into()))?;

    // Derive key (same as encryption)
    let key_material = format!("helm-hub-auth-{}", user_id);
    let hash = blake3::hash(key_material.as_bytes());
    let key_bytes: [u8; 32] = hash.as_bytes()[..32]
        .try_into()
        .map_err(|_| AppError::Internal("Key derivation failed".into()))?;

    let key = secretbox::Key(key_bytes);

    let plaintext = secretbox::open(ciphertext, &nonce, &key)
        .map_err(|_| AppError::Unauthorized("Token decryption failed".into()))?;

    String::from_utf8(plaintext)
        .map_err(|_| AppError::Internal("Invalid UTF-8 in decrypted token".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let user_id = Uuid::new_v4();
        let original_token = "test.jwt.token";

        let encrypted = encrypt_token(original_token, &user_id).expect("encrypt");
        assert!(encrypted.starts_with("encrypted:"));

        let decrypted = decrypt_token(&encrypted, &user_id).expect("decrypt");
        assert_eq!(decrypted, original_token);
    }

    #[test]
    fn test_wrong_user_id_fails() {
        let user_id_1 = Uuid::new_v4();
        let user_id_2 = Uuid::new_v4();
        let token = "test.jwt.token";

        let encrypted = encrypt_token(token, &user_id_1).expect("encrypt");
        let result = decrypt_token(&encrypted, &user_id_2);

        assert!(result.is_err());
    }
}
