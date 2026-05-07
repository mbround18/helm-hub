use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    /// Returned when ClamAV detects a virus in an uploaded file.
    #[error("Security violation: {0}")]
    InfectedFile(String),

    #[error("Rate limit exceeded: {0}")]
    TooManyRequests(String),

    #[error("Storage quota exceeded")]
    QuotaExceeded,

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Diesel(#[from] diesel::result::Error),

    #[error(transparent)]
    R2d2(#[from] r2d2::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, m.clone()),
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.clone()),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, m.clone()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::Conflict(m) => (StatusCode::CONFLICT, m.clone()),
            AppError::InfectedFile(m) => (StatusCode::FORBIDDEN, m.clone()),
            AppError::TooManyRequests(m) => (StatusCode::TOO_MANY_REQUESTS, m.clone()),
            AppError::QuotaExceeded => (
                StatusCode::PAYLOAD_TOO_LARGE,
                "Storage quota exceeded".into(),
            ),
            AppError::Diesel(diesel::result::Error::NotFound) => {
                (StatusCode::NOT_FOUND, "Record not found".into())
            }
            AppError::Diesel(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                info,
            )) => (
                StatusCode::CONFLICT,
                format!("Duplicate entry: {}", info.message()),
            ),
            _ => {
                tracing::error!(error = %self, "Unhandled internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".into(),
                )
            }
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}
