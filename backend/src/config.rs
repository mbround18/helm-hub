use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
    pub charts_storage_path: String,
    pub host: String,
    pub port: u16,
    /// Path to the clamd Unix socket.
    pub clamd_socket: String,
    /// When false the scan step is skipped entirely (useful in dev without ClamAV).
    pub clamav_enabled: bool,
    /// Directory for temporary upload files awaiting virus scan.
    pub temp_upload_dir: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            jwt_secret: env::var("JWT_SECRET").expect("JWT_SECRET must be set"),
            jwt_expiry_hours: env::var("JWT_EXPIRY_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24),
            charts_storage_path: env::var("CHARTS_STORAGE_PATH")
                .unwrap_or_else(|_| "./storage/charts".into()),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
            clamd_socket: env::var("CLAMD_SOCKET")
                .unwrap_or_else(|_| "/var/run/clamav/clamd.ctl".into()),
            clamav_enabled: env::var("CLAMAV_ENABLED")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(true),
            temp_upload_dir: env::var("TEMP_UPLOAD_DIR")
                .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned()),
        }
    }
}
