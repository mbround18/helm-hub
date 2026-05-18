pub mod admin;
pub mod jwt;
pub mod middleware;
pub mod password;
pub mod totp;

use rand::RngExt;
use sha2::{Digest, Sha256};

/// Generates a new API token: `hhub_<64 hex chars>` (256 bits of entropy).
pub fn generate_api_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("hhub_{hex}")
}

/// Computes the SHA-256 hex digest of a raw token string.
/// This is what gets stored in and compared against the database.
pub fn hash_api_token(token: &str) -> String {
    let hash = Sha256::digest(token.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}
