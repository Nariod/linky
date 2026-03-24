// Error handling module for Linky C2 Framework
// Uses thiserror for idiomatic error types and anyhow for propagation

use thiserror::Error;

#[derive(Error, Debug)]
pub enum LinkyError {
    #[error("build failed: {0}")]
    BuildFailed(String),

    #[error("link not found: {0}")]
    LinkNotFound(uuid::Uuid),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("actix-web error: {0}")]
    ActixWeb(#[from] actix_web::Error),

    #[error("rustls error: {0}")]
    Rustls(#[from] rustls::Error),

    #[error("chrono error: {0}")]
    Chrono(#[from] chrono::format::ParseError),

    #[error("unknown error")]
    Unknown,
}

pub type Result<T> = std::result::Result<T, LinkyError>;

// Conversion from anyhow::Error to LinkyError for compatibility
impl From<anyhow::Error> for LinkyError {
    fn from(err: anyhow::Error) -> Self {
        LinkyError::BuildFailed(err.to_string())
    }
}