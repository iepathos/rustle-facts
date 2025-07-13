use thiserror::Error;

#[derive(Error, Debug)]
pub enum FactsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("SSH error: {0}")]
    Ssh(String),

    #[error("SSH connection failed for host {0}: {1}")]
    ConnectionFailed(String, String),

    #[error("Authentication failed for host {0}")]
    AuthenticationFailed(String),

    #[error("Failed to parse facts from host {0}: {1}")]
    ParseError(String, String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Invalid inventory format: {0}")]
    InvalidInventory(String),

    #[error("Task join error: {0}")]
    TaskJoin(String),

    #[error("Timeout while gathering facts from host {0}")]
    Timeout(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

pub type Result<T> = std::result::Result<T, FactsError>;