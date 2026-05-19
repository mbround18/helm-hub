use sha2::{Digest, Sha256};
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
    pub charts_storage_path: String,
    pub host: String,
    pub port: u16,
    pub mgmt_port: u16,
    pub clamd_socket: String,
    pub clamav_enabled: bool,
    pub temp_upload_dir: String,
    pub static_assets_path: String,
    pub db_pool_size: u32,
    pub trust_proxy: bool,

    // ── Upload limits ─────────────────────────────────────────────────────────
    /// Maximum size of the entire multipart request body (bytes).
    pub max_upload_body_bytes: usize,
    /// Maximum size of a single chart .tgz file (bytes).
    pub max_chart_file_bytes: usize,

    // ── CORS ──────────────────────────────────────────────────────────────────
    /// Allowed browser origin for CORS responses (e.g. `https://helmhub.example.com`).
    pub frontend_origin: String,

    // ── Admin bootstrap ───────────────────────────────────────────────────────
    /// Username that is always treated as admin at runtime (no DB write needed).
    /// Set to the first operator account; rotate by updating the env var.
    pub admin_username: Option<String>,

    // ── Storage quotas ────────────────────────────────────────────────────────
    /// Global default quota in bytes (applies when per-user quota is NULL).
    /// Default: 5 GiB.
    pub default_storage_quota_bytes: i64,

    // ── GitHub OAuth App ──────────────────────────────────────────────────────
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub github_redirect_uri: Option<String>,

    // ── Security ──────────────────────────────────────────────────────────────
    /// Separate secret for signing OAuth state JWTs (distinct from session JWTs).
    /// Defaults to a domain-separated derivative of JWT_SECRET when not set.
    pub oauth_state_secret: String,

    /// 32-byte key for AES-256-GCM encryption of stored GitHub access tokens.
    /// Derived from JWT_SECRET when TOKEN_ENCRYPTION_KEY is not set.
    pub token_encryption_key: [u8; 32],

    // ── Observability ─────────────────────────────────────────────────────────
    /// Internal OTLP collector endpoint for backend traces/metrics.
    pub otel_collector_endpoint: Option<String>,
    pub log_format: String,
}

impl Config {
    pub fn from_env() -> Self {
        let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");

        if jwt_secret.len() < 32 {
            tracing::warn!(
                "JWT_SECRET is shorter than 32 characters. This is not recommended for production."
            );
        }

        // Derive oauth_state_secret: domain-separated SHA-256 of jwt_secret.
        let oauth_state_secret = env::var("OAUTH_STATE_SECRET").unwrap_or_else(|_| {
            let hash = Sha256::digest(format!("helm-hub-oauth-state:{jwt_secret}").as_bytes());
            hex::encode(hash)
        });

        // Derive token encryption key: 32-byte SHA-256 over a domain label + jwt_secret.
        let token_encryption_key: [u8; 32] = env::var("TOKEN_ENCRYPTION_KEY")
            .ok()
            .and_then(|k| hex::decode(&k).ok())
            .and_then(|b| b.try_into().ok())
            .unwrap_or_else(|| {
                let hash = Sha256::digest(format!("helm-hub-token-enc:{jwt_secret}").as_bytes());
                hash.into()
            });

        Self {
            jwt_secret,
            jwt_expiry_hours: env::var("JWT_EXPIRY_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            charts_storage_path: env::var("CHARTS_STORAGE_PATH")
                .unwrap_or_else(|_| "./storage/charts".into()),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3000),
            mgmt_port: env::var("MGMT_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(9090),
            clamd_socket: env::var("CLAMD_SOCKET")
                .unwrap_or_else(|_| "/var/run/clamav/clamd.ctl".into()),
            clamav_enabled: env::var("CLAMAV_ENABLED")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(true),
            temp_upload_dir: env::var("TEMP_UPLOAD_DIR")
                .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned()),
            static_assets_path: env::var("STATIC_ASSETS_PATH")
                .unwrap_or_else(|_| "/app/static".into()),
            db_pool_size: env::var("DB_POOL_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(16),
            trust_proxy: env::var("TRUST_PROXY")
                .map(|v| v.to_lowercase() == "true" || v == "1")
                .unwrap_or(false),

            max_upload_body_bytes: env::var("MAX_UPLOAD_BODY_MB")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(200)
                * 1024
                * 1024,
            max_chart_file_bytes: env::var("MAX_CHART_FILE_MB")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(50)
                * 1024
                * 1024,

            frontend_origin: env::var("FRONTEND_ORIGIN")
                .unwrap_or_else(|_| "http://localhost:5173".into()),

            admin_username: env::var("ADMIN_USERNAME").ok(),

            default_storage_quota_bytes: env::var("DEFAULT_STORAGE_QUOTA_GB")
                .ok()
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(5)
                * 1024
                * 1024
                * 1024,

            github_client_id: env::var("GITHUB_CLIENT_ID").ok(),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET").ok(),
            github_redirect_uri: env::var("GITHUB_REDIRECT_URI").ok(),

            oauth_state_secret,
            token_encryption_key,

            otel_collector_endpoint: env::var("OTEL_COLLECTOR_ENDPOINT").ok(),
            log_format: env::var("LOG_FORMAT").unwrap_or_else(|_| "text".into()),
        }
    }

    pub fn github_enabled(&self) -> bool {
        self.github_client_id.is_some()
            && self.github_client_secret.is_some()
            && self.github_redirect_uri.is_some()
    }

    pub fn print_summary(&self) {
        tracing::info!("── Configuration Summary ──────────────────────────────────");
        tracing::info!("Host: {}:{}", self.host, self.port);
        tracing::info!("Mgmt: {}:{}", self.host, self.mgmt_port);
        tracing::info!(
            "Database: {}",
            self.database_url.split('@').next_back().unwrap_or("unknown")
        );
        tracing::info!("Charts Storage: {}", self.charts_storage_path);
        tracing::info!("Static Assets: {}", self.static_assets_path);
        tracing::info!("Trust Proxy: {}", self.trust_proxy);
        tracing::info!(
            "ClamAV: {}",
            if self.clamav_enabled {
                format!("Enabled ({})", self.clamd_socket)
            } else {
                "Disabled".into()
            }
        );
        tracing::info!(
            "GitHub Sync: {}",
            if self.github_enabled() {
                "Enabled"
            } else {
                "Disabled"
            }
        );
        tracing::info!(
            "Admin User: {}",
            self.admin_username.as_deref().unwrap_or("None")
        );
        tracing::info!("Log Format: {}", self.log_format);
        tracing::info!(
            "Max Upload: {} MB",
            self.max_upload_body_bytes / 1024 / 1024
        );
        tracing::info!("Max Chart: {} MB", self.max_chart_file_bytes / 1024 / 1024);
        tracing::info!("───────────────────────────────────────────────────────────");
    }

    pub fn validate(&self) -> Result<(), String> {
        // ── 1. Database URL ──────────────────────────────────────────────────
        if !self.database_url.starts_with("postgres://")
            && !self.database_url.starts_with("postgresql://")
        {
            return Err("DATABASE_URL must be a postgres:// or postgresql:// URL".into());
        }

        // ── 2. Directory Writeability ────────────────────────────────────────
        let check_dir = |path: &str, label: &str| {
            let p = std::path::Path::new(path);
            if !p.exists() {
                std::fs::create_dir_all(p)
                    .map_err(|e| format!("Failed to create {label} directory '{path}': {e}"))?;
            }
            // Check writeability by creating/removing a temp file
            let temp = p.join(".write_test");
            std::fs::write(&temp, "ok")
                .map_err(|e| format!("{label} directory '{path}' is not writable: {e}"))?;
            let _ = std::fs::remove_file(temp);
            Ok::<(), String>(())
        };

        check_dir(&self.charts_storage_path, "Charts Storage")?;
        check_dir(&self.temp_upload_dir, "Temporary Upload")?;
        check_dir("/data", "Data")?;
        check_dir("/tmp/helm-uploads", "Temporary Upload Mount")?;

        // ── 3. Frontend Origin ───────────────────────────────────────────────
        if !self.frontend_origin.starts_with("http://")
            && !self.frontend_origin.starts_with("https://")
        {
            return Err(format!(
                "FRONTEND_ORIGIN '{}' must start with http:// or https://",
                self.frontend_origin
            ));
        }

        // ── 4. GitHub validation if enabled ──────────────────────────────────
        if self.github_enabled() && !self
                .github_redirect_uri
                .as_ref()
                .unwrap()
                .starts_with("http")
        {
            return Err("GITHUB_REDIRECT_URI must be an absolute URL".into());
        }

        Ok(())
    }
}
