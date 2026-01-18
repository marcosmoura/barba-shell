//! Unified error types for Stache.
//!
//! This module provides a hierarchical error system where each module can define
//! its own error type that converts into the base [`Error`] type.

use serde::Serialize;
use thiserror::Error;

/// Result type alias using our Error type.
pub type Result<T> = std::result::Result<T, Error>;

/// Base error type for all Stache errors.
///
/// This enum provides a unified error type that can represent errors from
/// any module in the application. Each module's specific error type can
/// be converted into this type using `From` implementations.
#[derive(Debug, Error)]
pub enum Error {
    /// Configuration-related errors.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Tiling window manager errors.
    #[error("Tiling error: {0}")]
    Tiling(String),

    /// Audio device management errors.
    #[error("Audio error: {0}")]
    Audio(String),

    /// Wallpaper management errors.
    #[error("Wallpaper error: {0}")]
    Wallpaper(String),

    /// Cache operation errors.
    #[error("Cache error: {0}")]
    Cache(String),

    /// IPC communication errors.
    #[error("IPC error: {0}")]
    Ipc(String),

    /// Battery information errors.
    #[error("Battery error: {0}")]
    Battery(String),

    /// Shell command execution errors.
    #[error("Shell error: {0}")]
    Shell(String),

    /// IO errors.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization errors.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Invalid arguments provided.
    #[error("{0}")]
    InvalidArguments(String),

    /// Generic error for uncategorized failures.
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Creates a configuration error.
    pub fn config(msg: impl Into<String>) -> Self { Self::Config(msg.into()) }

    /// Creates a tiling error.
    pub fn tiling(msg: impl Into<String>) -> Self { Self::Tiling(msg.into()) }

    /// Creates an audio error.
    pub fn audio(msg: impl Into<String>) -> Self { Self::Audio(msg.into()) }

    /// Creates a wallpaper error.
    pub fn wallpaper(msg: impl Into<String>) -> Self { Self::Wallpaper(msg.into()) }

    /// Creates a cache error.
    pub fn cache(msg: impl Into<String>) -> Self { Self::Cache(msg.into()) }

    /// Creates an IPC error.
    pub fn ipc(msg: impl Into<String>) -> Self { Self::Ipc(msg.into()) }

    /// Creates an invalid arguments error.
    pub fn invalid_args(msg: impl Into<String>) -> Self { Self::InvalidArguments(msg.into()) }

    /// Creates a generic error.
    pub fn other(msg: impl Into<String>) -> Self { Self::Other(msg.into()) }
}

impl From<String> for Error {
    fn from(msg: String) -> Self { Self::Other(msg) }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self { Self::Other(msg.to_string()) }
}

// ============================================================================
// Legacy StacheError (for backward compatibility with existing code)
// ============================================================================

/// Errors that can occur during application execution.
///
/// This enum implements `Serialize` and `Into<tauri::ipc::InvokeError>` to be
/// used as a return type for Tauri commands, providing structured error information
/// to the frontend.
///
/// **Note**: This is maintained for backward compatibility. New code should use [`Error`].
#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum StacheError {
    /// Invalid command arguments.
    #[error("{0}")]
    InvalidArguments(String),
    /// Cache operation failed.
    #[error("Cache error: {0}")]
    CacheError(String),
    /// Audio operation failed.
    #[error("Audio error: {0}")]
    AudioError(String),
    /// Wallpaper operation failed.
    #[error("Wallpaper error: {0}")]
    WallpaperError(String),
    /// Configuration error.
    #[error("Configuration error: {0}")]
    ConfigError(String),
    /// IPC communication error.
    #[error("IPC error: {0}")]
    IpcError(String),
    /// IO error.
    #[error("IO error: {0}")]
    IoError(String),
    /// Battery operation failed.
    #[error("Battery error: {0}")]
    BatteryError(String),
    /// Tiling window manager operation failed.
    #[error("Tiling error: {0}")]
    TilingError(String),
    /// Shell command execution failed.
    #[error("Shell error: {0}")]
    ShellError(String),
    /// Generic command error.
    #[error("{0}")]
    CommandError(String),
}

impl From<std::io::Error> for StacheError {
    fn from(err: std::io::Error) -> Self { Self::IoError(err.to_string()) }
}

impl From<serde_json::Error> for StacheError {
    fn from(err: serde_json::Error) -> Self { Self::CommandError(err.to_string()) }
}

impl From<String> for StacheError {
    fn from(msg: String) -> Self { Self::CommandError(msg) }
}

impl From<&str> for StacheError {
    fn from(msg: &str) -> Self { Self::CommandError(msg.to_string()) }
}

impl From<Error> for StacheError {
    fn from(err: Error) -> Self {
        match err {
            Error::Config(msg) => Self::ConfigError(msg),
            Error::Tiling(msg) => Self::TilingError(msg),
            Error::Audio(msg) => Self::AudioError(msg),
            Error::Wallpaper(msg) => Self::WallpaperError(msg),
            Error::Cache(msg) => Self::CacheError(msg),
            Error::Ipc(msg) => Self::IpcError(msg),
            Error::Battery(msg) => Self::BatteryError(msg),
            Error::Shell(msg) => Self::ShellError(msg),
            Error::Io(err) => Self::IoError(err.to_string()),
            Error::Json(err) => Self::CommandError(err.to_string()),
            Error::InvalidArguments(msg) => Self::InvalidArguments(msg),
            Error::Other(msg) => Self::CommandError(msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::config("Invalid JSON");
        assert_eq!(err.to_string(), "Configuration error: Invalid JSON");
    }

    #[test]
    fn test_error_from_string() {
        let err: Error = "test error".into();
        assert!(matches!(err, Error::Other(_)));
    }

    #[test]
    fn test_stache_error_serializes_with_kind() {
        let err = StacheError::BatteryError("No battery".to_string());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("BatteryError"));
        assert!(json.contains("No battery"));
    }

    #[test]
    fn test_error_to_stache_error_conversion() {
        let err = Error::tiling("Workspace not found");
        let stache_err: StacheError = err.into();
        assert!(matches!(stache_err, StacheError::TilingError(_)));
    }
}
