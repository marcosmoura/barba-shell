//! Tauri commands for the tiling window manager.
//!
//! This module exposes tiling functionality to the frontend via Tauri commands.

use barba_shared::WorkspaceInfo;

use super::manager::try_get_manager;

/// Returns the list of all workspaces with their current state.
///
/// Each workspace includes its name, layout mode, screen, focus state, and window count.
#[tauri::command]
#[allow(clippy::significant_drop_tightening)]
pub fn get_workspaces() -> Result<Vec<WorkspaceInfo>, String> {
    let manager_lock =
        try_get_manager().ok_or_else(|| "Tiling manager not initialized".to_string())?;

    let manager = manager_lock.read();
    let state = manager.workspace_manager.state();

    let focused_workspace = state.focused_workspace.as_deref();
    let focused_window = state.focused_window;

    let workspaces: Vec<WorkspaceInfo> = state
        .workspaces
        .iter()
        .map(|ws| {
            ws.to_info(
                focused_workspace == Some(&ws.name),
                &state.screens,
                &state.windows,
                focused_window,
            )
        })
        .collect();

    Ok(workspaces)
}

/// Switches to a workspace by name.
///
/// This will hide windows from the current workspace on the same screen
/// and show windows from the target workspace.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn switch_workspace(name: String) -> Result<(), String> {
    let manager_lock =
        try_get_manager().ok_or_else(|| "Tiling manager not initialized".to_string())?;

    let mut manager = manager_lock.write();
    manager.switch_workspace(&name).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    // Tests would require mocking the global tiling manager,
    // which is tested in the manager module itself.
}
