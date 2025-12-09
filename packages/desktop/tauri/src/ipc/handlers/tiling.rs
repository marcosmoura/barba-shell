//! Tiling window manager command handlers.
//!
//! This module handles all tiling-related IPC commands including:
//! - Query commands (screens, workspaces, windows)
//! - Workspace operations (focus, layout, balance, send-to-screen)
//! - Window operations (move, focus, send-to-workspace, send-to-screen, resize, presets)

use std::os::unix::net::UnixStream;

use barba_shared::LayoutMode;
use serde::Deserialize;

use crate::ipc::server::write_json_response;
use crate::ipc::types::{Direction, ResizeDimension};
use crate::tiling;

// ============================================================================
// Query Data Types
// ============================================================================

/// Data for query workspaces command.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryWorkspacesData {
    #[serde(default)]
    focused: bool,
    name: Option<String>,
    #[serde(default)]
    focused_screen: bool,
    screen: Option<String>,
}

/// Data for query windows command.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueryWindowsData {
    #[serde(default)]
    focused_workspace: bool,
    #[serde(default)]
    focused_screen: bool,
    workspace: Option<String>,
    screen: Option<String>,
}

// ============================================================================
// Workspace Data Types
// ============================================================================

/// Data for workspace focus command.
#[derive(Deserialize)]
struct WorkspaceFocusData {
    target: String,
}

/// Data for workspace layout command.
#[derive(Deserialize)]
struct WorkspaceLayoutData {
    layout: String,
}

/// Data for workspace send to screen command.
#[derive(Deserialize)]
struct WorkspaceSendToScreenData {
    screen: String,
}

// ============================================================================
// Window Data Types
// ============================================================================

/// Data for window move command.
#[derive(Deserialize)]
struct WindowMoveData {
    direction: Direction,
}

/// Data for window focus command.
#[derive(Deserialize)]
struct WindowFocusData {
    direction: Direction,
}

/// Data for window send to workspace command.
#[derive(Deserialize)]
struct WindowSendToWorkspaceData {
    workspace: String,
    #[serde(default = "default_true")]
    focus: bool,
}

const fn default_true() -> bool { true }

/// Data for window send to screen command.
#[derive(Deserialize)]
struct WindowSendToScreenData {
    screen: String,
}

/// Data for window resize command.
#[derive(Deserialize)]
struct WindowResizeData {
    dimension: ResizeDimension,
    amount: i32,
}

/// Data for window preset command.
#[derive(Deserialize)]
struct WindowPresetData {
    name: String,
}

// ============================================================================
// Query Handlers
// ============================================================================

/// Handles the tiling-query-screens command.
///
/// Returns JSON array of screen information.
pub fn handle_query_screens(stream: &mut UnixStream) {
    let response = match tiling::screen::get_all_screens() {
        Ok(screens) => {
            let screen_count = screens.len();
            let infos: Vec<_> = screens.iter().map(|s| s.to_info(screen_count)).collect();
            serde_json::to_string(&infos).unwrap_or_else(|_| "[]".to_string())
        }
        Err(err) => {
            eprintln!("barba: tiling query screens error: {err:?}");
            "[]".to_string()
        }
    };
    write_json_response(stream, &response);
}

/// Handles the tiling-query-workspaces command.
///
/// Returns JSON array of workspace information, optionally filtered.
#[allow(clippy::significant_drop_tightening)]
pub fn handle_query_workspaces(stream: &mut UnixStream, data: Option<&str>) {
    let Some(manager_lock) = tiling::try_get_manager() else {
        write_json_response(stream, "[]");
        return;
    };
    let filter = data.and_then(|d| serde_json::from_str::<QueryWorkspacesData>(d).ok());

    let manager = manager_lock.read();
    let state = manager.workspace_manager.state();

    let workspaces: Vec<_> = state
        .workspaces
        .iter()
        .filter(|ws| {
            if let Some(ref f) = filter {
                // Filter by focused workspace
                if f.focused {
                    return state.focused_workspace.as_ref() == Some(&ws.name);
                }
                // Filter by workspace name
                if let Some(ref name) = f.name {
                    return &ws.name == name;
                }
                if f.focused_screen {
                    // Filter by focused screen
                    if let Some(focused_ws) = state.focused_workspace.as_ref()
                        && let Some(focused) = state.get_workspace(focused_ws)
                    {
                        return ws.screen == focused.screen;
                    }
                    return false;
                }
                if let Some(ref screen) = f.screen {
                    return ws.screen == *screen;
                }
            }
            true
        })
        .map(|ws| {
            ws.to_info(
                state.focused_workspace.as_ref() == Some(&ws.name),
                &state.screens,
                &state.windows,
                state.focused_window,
            )
        })
        .collect();

    let response = serde_json::to_string(&workspaces).unwrap_or_else(|_| "[]".to_string());
    write_json_response(stream, &response);
}

/// Handles the tiling-query-windows command.
///
/// Returns JSON array of window information, optionally filtered.
#[allow(clippy::significant_drop_tightening)]
pub fn handle_query_windows(stream: &mut UnixStream, data: Option<&str>) {
    let Some(manager_lock) = tiling::try_get_manager() else {
        write_json_response(stream, "[]");
        return;
    };
    let filter = data.and_then(|d| serde_json::from_str::<QueryWindowsData>(d).ok());

    let manager = manager_lock.read();
    let state = manager.workspace_manager.state();

    let windows: Vec<_> = state
        .windows
        .values()
        .filter(|window| {
            if let Some(ref f) = filter {
                if f.focused_workspace {
                    if let Some(ref focused_ws) = state.focused_workspace {
                        return &window.workspace == focused_ws;
                    }
                    return false;
                }
                if f.focused_screen {
                    if let Some(ref focused_ws) = state.focused_workspace
                        && let Some(focused) = state.get_workspace(focused_ws)
                    {
                        return state
                            .get_workspace(&window.workspace)
                            .is_some_and(|ws| ws.screen == focused.screen);
                    }
                    return false;
                }
                if let Some(ref ws) = f.workspace {
                    return &window.workspace == ws;
                }
                if let Some(ref screen) = f.screen {
                    return state
                        .get_workspace(&window.workspace)
                        .is_some_and(|ws| &ws.screen == screen);
                }
            }
            true
        })
        .map(|window| window.to_info(state.focused_window == Some(window.id)))
        .collect();

    let response = serde_json::to_string(&windows).unwrap_or_else(|_| "[]".to_string());
    write_json_response(stream, &response);
}

// ============================================================================
// Workspace Handlers
// ============================================================================

/// Handles the tiling-workspace-focus command.
///
/// Focuses a workspace by name, direction (next/previous), or screen direction.
pub fn handle_workspace_focus(data: &str) {
    let Ok(focus_data) = serde_json::from_str::<WorkspaceFocusData>(data) else {
        eprintln!("barba: failed to parse workspace focus data");
        return;
    };

    let Some(manager_lock) = tiling::try_get_manager() else {
        eprintln!("barba: tiling not initialized");
        return;
    };
    let mut manager = manager_lock.write();

    // Check if target is a direction
    let result = match focus_data.target.to_lowercase().as_str() {
        "next" => focus_next_workspace(&mut manager),
        "previous" | "prev" => focus_previous_workspace(&mut manager),
        // Directional focus - switch to the focused workspace on an adjacent screen
        "up" | "down" | "left" | "right" => {
            manager.focus_workspace_on_screen(&focus_data.target.to_lowercase())
        }
        // Direct workspace name
        _ => manager.switch_workspace(&focus_data.target),
    };

    if let Err(err) = result {
        eprintln!("barba: workspace focus error: {err:?}");
    }
}

/// Focus the next workspace on the current screen.
fn focus_next_workspace(
    manager: &mut parking_lot::RwLockWriteGuard<tiling::manager::TilingManager>,
) -> Result<(), tiling::TilingError> {
    let current = manager.workspace_manager.state().focused_workspace.clone();
    if let Some(current_name) = current {
        if let Some(current_ws) = manager.workspace_manager.state().get_workspace(&current_name) {
            let screen_id = current_ws.screen.clone();
            let workspaces_on_screen =
                manager.workspace_manager.state().get_workspaces_on_screen(&screen_id);
            let current_idx =
                workspaces_on_screen.iter().position(|w| w.name == current_name).unwrap_or(0);
            let next_idx = (current_idx + 1) % workspaces_on_screen.len();
            let next_name = workspaces_on_screen[next_idx].name.clone();
            manager.switch_workspace(&next_name)
        } else {
            Err(tiling::TilingError::WorkspaceNotFound(current_name))
        }
    } else {
        Err(tiling::TilingError::WorkspaceNotFound("current".to_string()))
    }
}

/// Focus the previous workspace on the current screen.
fn focus_previous_workspace(
    manager: &mut parking_lot::RwLockWriteGuard<tiling::manager::TilingManager>,
) -> Result<(), tiling::TilingError> {
    let current = manager.workspace_manager.state().focused_workspace.clone();
    if let Some(current_name) = current {
        if let Some(current_ws) = manager.workspace_manager.state().get_workspace(&current_name) {
            let screen_id = current_ws.screen.clone();
            let workspaces_on_screen =
                manager.workspace_manager.state().get_workspaces_on_screen(&screen_id);
            let current_idx =
                workspaces_on_screen.iter().position(|w| w.name == current_name).unwrap_or(0);
            let prev_idx = if current_idx == 0 {
                workspaces_on_screen.len() - 1
            } else {
                current_idx - 1
            };
            let prev_name = workspaces_on_screen[prev_idx].name.clone();
            manager.switch_workspace(&prev_name)
        } else {
            Err(tiling::TilingError::WorkspaceNotFound(current_name))
        }
    } else {
        Err(tiling::TilingError::WorkspaceNotFound("current".to_string()))
    }
}

/// Handles the tiling-workspace-layout command.
///
/// Sets the layout mode for the currently focused workspace.
pub fn handle_workspace_layout(data: &str) {
    let Ok(layout_data) = serde_json::from_str::<WorkspaceLayoutData>(data) else {
        eprintln!("barba: failed to parse workspace layout data");
        return;
    };

    let Ok(layout_mode) = layout_data.layout.parse::<LayoutMode>() else {
        eprintln!("barba: invalid layout mode: {}", layout_data.layout);
        return;
    };

    let Some(manager_lock) = tiling::try_get_manager() else {
        eprintln!("barba: tiling not initialized");
        return;
    };
    let mut manager = manager_lock.write();
    let focused_ws = manager.workspace_manager.state().focused_workspace.clone();

    if let Some(ws_name) = focused_ws {
        if let Err(err) = manager.set_workspace_layout(&ws_name, layout_mode) {
            eprintln!("barba: set layout error: {err:?}");
        }
    } else {
        eprintln!("barba: no focused workspace");
    }
}

/// Handles the tiling-workspace-balance command.
///
/// Balances window sizes in the focused workspace by resetting split ratios
/// to their default values and re-applying the layout.
pub fn handle_workspace_balance() {
    let Some(manager_lock) = tiling::try_get_manager() else {
        eprintln!("barba: tiling not initialized");
        return;
    };

    let mut manager = manager_lock.write();

    // Get the focused workspace name
    let Some(ws_name) = manager.workspace_manager.state().focused_workspace.clone() else {
        eprintln!("barba: no focused workspace");
        return;
    };

    // Balance the workspace (clears split ratios and re-applies layout)
    if let Err(err) = manager.balance_workspace(&ws_name) {
        eprintln!("barba: balance workspace error: {err:?}");
    }
}

/// Handles the tiling-workspace-send-to-screen command.
///
/// Sends the focused workspace to a different screen.
pub fn handle_workspace_send_to_screen(data: &str) {
    let Ok(send_data) = serde_json::from_str::<WorkspaceSendToScreenData>(data) else {
        eprintln!("barba: failed to parse workspace send-to-screen data");
        return;
    };

    let Some(manager_lock) = tiling::try_get_manager() else {
        eprintln!("barba: tiling not initialized");
        return;
    };
    let mut manager = manager_lock.write();

    if let Err(err) = manager.send_workspace_to_screen(&send_data.screen) {
        eprintln!("barba: send workspace to screen error: {err:?}");
    }
}

// ============================================================================
// Window Handlers
// ============================================================================

/// Handles the tiling-window-move command.
///
/// Swaps the focused window with the window in the specified direction.
pub fn handle_window_move(data: &str) {
    let Ok(move_data) = serde_json::from_str::<WindowMoveData>(data) else {
        eprintln!("barba: failed to parse window move data");
        return;
    };

    let Some(manager_lock) = tiling::try_get_manager() else {
        eprintln!("barba: tiling not initialized");
        return;
    };
    let mut manager = manager_lock.write();

    if let Err(err) = manager.swap_window_in_direction(move_data.direction.as_str()) {
        eprintln!("barba: move window error: {err:?}");
    }
}

/// Handles the tiling-window-focus command.
///
/// Focuses the window in the specified direction.
pub fn handle_window_focus(data: &str) {
    let Ok(focus_data) = serde_json::from_str::<WindowFocusData>(data) else {
        eprintln!("barba: failed to parse window focus data");
        return;
    };

    let Some(manager_lock) = tiling::try_get_manager() else {
        eprintln!("barba: tiling not initialized");
        return;
    };
    let mut manager = manager_lock.write();

    if let Err(err) = manager.focus_window_in_direction(focus_data.direction.as_str()) {
        eprintln!("barba: focus window error: {err:?}");
    }
}

/// Handles the tiling-window-send-to-workspace command.
///
/// Sends the focused window to a specific workspace.
pub fn handle_window_send_to_workspace(data: &str) {
    let Ok(send_data) = serde_json::from_str::<WindowSendToWorkspaceData>(data) else {
        eprintln!("barba: failed to parse window send-to-workspace data");
        return;
    };

    let Some(manager_lock) = tiling::try_get_manager() else {
        eprintln!("barba: tiling not initialized");
        return;
    };
    let mut manager = manager_lock.write();

    if let Err(err) =
        manager.send_window_to_workspace_with_options(&send_data.workspace, send_data.focus)
    {
        eprintln!("barba: send window to workspace error: {err:?}");
    }
}

/// Handles the tiling-window-send-to-screen command.
///
/// Sends the focused window to a different screen.
pub fn handle_window_send_to_screen(data: &str) {
    let Ok(send_data) = serde_json::from_str::<WindowSendToScreenData>(data) else {
        eprintln!("barba: failed to parse window send-to-screen data");
        return;
    };

    let Some(manager_lock) = tiling::try_get_manager() else {
        eprintln!("barba: tiling not initialized");
        return;
    };
    let mut manager = manager_lock.write();

    if let Err(err) = manager.send_window_to_screen(&send_data.screen) {
        eprintln!("barba: send window to screen error: {err:?}");
    }
}

/// Handles the tiling-window-resize command.
///
/// Resizes the focused window by the specified amount.
/// For tiled layouts: adjusts split ratios and re-applies the layout.
/// For floating layouts: directly resizes the window.
/// This operation is ignored for:
/// - Windows in monocle layout (they always fill the screen)
/// - Unmanaged windows (not tracked by the tiling manager)
pub fn handle_window_resize(data: &str) {
    let Ok(resize_data) = serde_json::from_str::<WindowResizeData>(data) else {
        eprintln!("barba: failed to parse window resize data");
        return;
    };

    let Some(manager_lock) = tiling::try_get_manager() else {
        // Tiling not initialized - window is unmanaged
        return;
    };

    let mut manager = manager_lock.write();

    let dimension = match resize_data.dimension {
        ResizeDimension::Width => "width",
        ResizeDimension::Height => "height",
    };

    if let Err(err) = manager.resize_focused_window(dimension, resize_data.amount) {
        eprintln!("barba: failed to resize window: {err:?}");
    }
}

/// Handles the tiling-window-preset command.
///
/// Applies a floating preset to the focused window.
pub fn handle_window_preset(data: &str) {
    let Ok(preset_data) = serde_json::from_str::<WindowPresetData>(data) else {
        eprintln!("barba: failed to parse window preset data");
        return;
    };

    let Some(manager_lock) = tiling::try_get_manager() else {
        eprintln!("barba: tiling not initialized");
        return;
    };

    let mut manager = manager_lock.write();

    if let Err(err) = manager.apply_preset(&preset_data.name) {
        eprintln!("barba: failed to apply preset '{}': {err:?}", preset_data.name);
    }
}

/// Handles the tiling-window-close command.
///
/// Closes the focused window using the Accessibility API.
pub fn handle_window_close() {
    let Some(manager_lock) = tiling::try_get_manager() else {
        eprintln!("barba: tiling not initialized");
        return;
    };

    let manager = manager_lock.read();
    let state = manager.workspace_manager.state();

    // Get the focused window
    let Some(focused_window_id) = state.focused_window else {
        eprintln!("barba: no focused window");
        return;
    };

    // Release lock before calling close_window (which may trigger observer callbacks)
    drop(manager);

    if let Err(err) = tiling::window::close_window(focused_window_id) {
        eprintln!("barba: failed to close window: {err:?}");
    }
}
