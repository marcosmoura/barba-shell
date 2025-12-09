//! IPC command handlers.
//!
//! This module provides the dispatch logic for routing IPC commands
//! to their appropriate handlers.

pub mod system;
pub mod tiling;
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
        "generate-schema" => {
            system::handle_generate_schema(stream);
            Some(true)
        }

        // Tiling query commands
        "tiling-query-screens" => {
            tiling::handle_query_screens(stream);
            Some(true)
        }
        "tiling-query-workspaces" => {
            tiling::handle_query_workspaces(stream, payload.data.as_deref());
            Some(true)
        }
        "tiling-query-windows" => {
            tiling::handle_query_windows(stream, payload.data.as_deref());
            Some(true)
        }

        // Tiling workspace commands
        "tiling-workspace-focus" => {
            if let Some(data) = &payload.data {
                tiling::handle_workspace_focus(data);
            }
            stream.write_all(b"1").ok();
            Some(true)
        }
        "tiling-workspace-layout" => {
            if let Some(data) = &payload.data {
                tiling::handle_workspace_layout(data);
            }
            stream.write_all(b"1").ok();
            Some(true)
        }
        "tiling-workspace-balance" => {
            tiling::handle_workspace_balance();
            stream.write_all(b"1").ok();
            Some(true)
        }
        "tiling-workspace-send-to-screen" => {
            if let Some(data) = &payload.data {
                tiling::handle_workspace_send_to_screen(data);
            }
            stream.write_all(b"1").ok();
            Some(true)
        }

        // Tiling window commands
        "tiling-window-move" => {
            if let Some(data) = &payload.data {
                tiling::handle_window_move(data);
            }
            stream.write_all(b"1").ok();
            Some(true)
        }
        "tiling-window-focus" => {
            if let Some(data) = &payload.data {
                tiling::handle_window_focus(data);
            }
            stream.write_all(b"1").ok();
            Some(true)
        }
        "tiling-window-send-to-workspace" => {
            if let Some(data) = &payload.data {
                tiling::handle_window_send_to_workspace(data);
            }
            stream.write_all(b"1").ok();
            Some(true)
        }
        "tiling-window-send-to-screen" => {
            if let Some(data) = &payload.data {
                tiling::handle_window_send_to_screen(data);
            }
            stream.write_all(b"1").ok();
            Some(true)
        }
        "tiling-window-resize" => {
            if let Some(data) = &payload.data {
                tiling::handle_window_resize(data);
            }
            stream.write_all(b"1").ok();
            Some(true)
        }
        "tiling-window-preset" => {
            if let Some(data) = &payload.data {
                tiling::handle_window_preset(data);
            }
            stream.write_all(b"1").ok();
            Some(true)
        }
        "tiling-window-close" => {
            tiling::handle_window_close();
            stream.write_all(b"1").ok();
            Some(true)
        }

        // Unknown command - let the caller forward it to the frontend
        _ => None,
    }
}
