use serde_json::Error as JsonError;
use std::io::Error as IOError;
use thiserror::Error;

/// Common error type.
#[derive(Error, Debug)]
pub enum Error {
    /// Missing environment variable.
    #[error("missing environment variable: {0}")]
    MissingEnvVar(&'static str),
    /// I/O error.
    #[error("IO Error")]
    IO(#[from] IOError),
    /// JSON error.
    #[error("Json Error")]
    Json(#[from] JsonError),
    /// Bad configuration.
    #[error("bad configuration: {0}")]
    BadConfig(String),
}

/// Type alias for Results which may return [`Error`].
pub type Result<T> = std::result::Result<T, Error>;
