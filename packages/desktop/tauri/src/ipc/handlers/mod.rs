//! IPC command handlers.
//!
//! This module provides the dispatch logic for routing IPC commands
//! to their appropriate handlers.

pub mod system;
pub mod wallpaper;

use std::io::Write;
use std::os::unix::net::UnixStream;

use super::types::IpcPayload;

/// Dispatches an IPC command to the appropriate handler.
///
/// Returns `Some(true)` if the command was handled successfully,
/// `Some(false)` if it was handled but failed,
/// or `None` if the command should be forwarded to the frontend.
#[allow(clippy::too_many_lines)]
pub fn dispatch(payload: &IpcPayload, stream: &mut UnixStream) -> Option<bool> {
    match payload.name.as_str() {
        // Wallpaper commands
        "wallpaper-set" => {
            if let Some(data) = &payload.data {
                wallpaper::handle_set(data);
            }
            stream.write_all(b"1").ok();
            Some(true)
        }
        "wallpaper-generate-all" => Some(wallpaper::handle_generate_all(stream)),
        "wallpaper-list" => {
            wallpaper::handle_list(stream);
            Some(true)
        }

        // System commands
        "schema" => {
            system::handle_generate_schema(stream);
            Some(true)
        }

        // Unknown command - let the caller forward it to the frontend
        _ => None,
    }
}
