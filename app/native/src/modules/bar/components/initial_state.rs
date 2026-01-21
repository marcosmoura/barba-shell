//! Initial state command for batched data fetching.
//!
//! This module provides a single command that returns all initial UI state
//! in one IPC call, reducing startup latency.

use serde::Serialize;
use serde_json::Value;

use super::battery::{self, BatteryInfo};
use super::cpu::{self, CpuInfo};
use super::tiling::{self, WindowInfo, WorkspaceInfo};
use super::weather::{self, WeatherConfigInfo};
use crate::modules::tiling as tiling_module;

/// Initial state for the tiling window manager.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TilingInitialState {
    /// All workspaces.
    pub workspaces: Vec<WorkspaceInfo>,
    /// Currently focused workspace name.
    pub focused_workspace: Option<String>,
    /// Windows in the current workspace.
    pub current_workspace_windows: Vec<WindowInfo>,
    /// Currently focused window.
    pub focused_window: Option<WindowInfo>,
}

/// Batched initial state for the UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialState {
    /// Battery information.
    pub battery: Option<BatteryInfo>,
    /// CPU information.
    pub cpu: Option<CpuInfo>,
    /// Current media playback info.
    pub media: Option<Value>,
    /// Weather configuration.
    pub weather_config: Option<WeatherConfigInfo>,
    /// Tiling state (if enabled and initialized).
    pub tiling: Option<TilingInitialState>,
}

/// Returns all initial UI state in a single IPC call.
///
/// This reduces startup latency by batching multiple queries into one round-trip.
#[tauri::command]
pub async fn get_initial_state(app: tauri::AppHandle) -> InitialState {
    // Fetch tiling state if enabled and initialized
    let tiling = if tiling_module::init::is_enabled() && tiling_module::init::is_initialized() {
        // These are all async, run them concurrently
        let (workspaces, focused_workspace, current_windows, focused_window) = tokio::join!(
            tiling::get_tiling_workspaces(None),
            tiling::get_tiling_focused_workspace(),
            tiling::get_tiling_current_workspace_windows(),
            tiling::get_tiling_focused_window(),
        );

        Some(TilingInitialState {
            workspaces: workspaces.unwrap_or_default(),
            focused_workspace: focused_workspace.ok().flatten(),
            current_workspace_windows: current_windows.unwrap_or_default(),
            focused_window: focused_window.ok().flatten(),
        })
    } else {
        None
    };

    // Run synchronous functions that use block_on internally on a blocking thread
    // to avoid "Cannot start a runtime from within a runtime" panic
    let app_clone = app.clone();
    let (battery, cpu) = tokio::join!(
        tokio::task::spawn_blocking({
            let app = app.clone();
            move || battery::get_battery_info(app).ok()
        }),
        tokio::task::spawn_blocking(move || cpu::get_cpu_info(app_clone)),
    );

    InitialState {
        battery: battery.ok().flatten(),
        cpu: cpu.ok(),
        media: super::media::get_current_media_info(),
        weather_config: Some(weather::get_weather_config()),
        tiling,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_serializes_correctly() {
        let state = InitialState {
            battery: None,
            cpu: None,
            media: None,
            weather_config: None,
            tiling: None,
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"battery\":null"));
        assert!(json.contains("\"cpu\":null"));
        assert!(json.contains("\"media\":null"));
        assert!(json.contains("\"weatherConfig\":null"));
        assert!(json.contains("\"tiling\":null"));
    }

    #[test]
    fn test_tiling_initial_state_serializes_correctly() {
        let tiling_state = TilingInitialState {
            workspaces: vec![],
            focused_workspace: Some("terminal".to_string()),
            current_workspace_windows: vec![],
            focused_window: None,
        };

        let json = serde_json::to_string(&tiling_state).unwrap();
        assert!(json.contains("\"workspaces\":[]"));
        assert!(json.contains("\"focusedWorkspace\":\"terminal\""));
        assert!(json.contains("\"currentWorkspaceWindows\":[]"));
        assert!(json.contains("\"focusedWindow\":null"));
    }
}
