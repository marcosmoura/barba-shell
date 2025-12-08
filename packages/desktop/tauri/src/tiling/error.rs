//! Error types for the tiling window manager.

use std::fmt;

/// Errors that can occur in the tiling window manager.
#[derive(Debug)]
pub enum TilingError {
    /// Accessibility permissions are not granted.
    AccessibilityNotAuthorized,

    /// Window not found.
    WindowNotFound(u64),

    /// Workspace not found.
    WorkspaceNotFound(String),

    /// Screen not found.
    ScreenNotFound(String),

    /// A general operation failed.
    OperationFailed(String),

    /// Failed to start window observer.
    ObserverFailed(String),
}

impl fmt::Display for TilingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AccessibilityNotAuthorized => {
                write!(f, "Accessibility permissions not granted")
            }
            Self::WindowNotFound(id) => {
                write!(f, "Window not found: {id}")
            }
            Self::WorkspaceNotFound(name) => {
                write!(f, "Workspace not found: {name}")
            }
            Self::ScreenNotFound(name) => {
                write!(f, "Screen not found: {name}")
            }
            Self::OperationFailed(msg) => {
                write!(f, "Operation failed: {msg}")
            }
            Self::ObserverFailed(msg) => {
                write!(f, "Failed to start observer: {msg}")
            }
        }
    }
}

impl std::error::Error for TilingError {}
