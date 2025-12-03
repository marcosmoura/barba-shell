//! Shared types and constants for the CLI module.

use serde::Serialize;

/// Channel name for CLI events emitted to the frontend.
pub const CLI_EVENT_CHANNEL: &str = "tauri_cli_event";

/// Synthetic binary name used for CLI argument normalization.
pub const SYNTHETIC_BIN_NAME: &str = "barba";

/// Application version from Cargo.toml.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Payload for CLI events emitted to the frontend.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CliEventPayload {
    /// The name of the CLI command/event.
    pub name: String,
    /// Optional data associated with the command.
    pub data: Option<String>,
}
