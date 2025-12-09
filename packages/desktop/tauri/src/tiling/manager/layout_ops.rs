//! Layout application and management.
//!
//! This module handles applying layouts to workspaces, including
//! computing window positions and sizes.

use std::collections::{HashMap, HashSet};

use barba_shared::LayoutMode;

use super::TilingManager;
use crate::tiling::error::TilingError;
use crate::tiling::layout::{self, Layout, ResolvedGaps};
use crate::tiling::state::ManagedWindow;
use crate::tiling::window;

impl TilingManager {
    /// Applies layouts to all workspaces.
    pub(super) fn apply_all_layouts(&mut self) {
        let workspace_names: Vec<String> = self
            .workspace_manager
            .state()
            .workspaces
            .iter()
            .map(|ws| ws.name.clone())
            .collect();

        for ws_name in workspace_names {
            if let Err(e) = self.apply_layout(&ws_name) {
                eprintln!("barba: failed to apply layout to workspace '{ws_name}': {e}");
            }
        }
    }

    /// Applies the layout to a workspace.
    pub fn apply_layout(&mut self, workspace_name: &str) -> Result<(), TilingError> {
        // Get the current list of actual windows from the system
        let actual_windows: Vec<ManagedWindow> =
            window::get_all_windows_including_hidden().unwrap_or_default();
        let actual_window_ids: HashSet<u64> = actual_windows.iter().map(|w| w.id).collect();

        let workspace = self
            .workspace_manager
            .state()
            .get_workspace(workspace_name)
            .ok_or_else(|| TilingError::WorkspaceNotFound(workspace_name.to_string()))?;

        let screen_id = workspace.screen.clone();
        let layout_mode = workspace.layout.clone();
        let workspace_window_ids = workspace.windows.clone();

        // Get the apps assigned to this workspace from rules
        let workspace_app_ids = self.get_workspace_app_ids(workspace_name);

        // Count windows per app to detect multi-window apps
        let app_window_counts = Self::count_windows_per_app(&actual_windows, &workspace_app_ids);

        // Filter to valid window IDs for this workspace
        let window_ids = self.filter_valid_window_ids(
            &workspace_window_ids,
            &actual_windows,
            &actual_window_ids,
            &workspace_app_ids,
            &app_window_counts,
            workspace_name,
            &layout_mode,
        );

        // Clean up stale windows
        self.cleanup_stale_windows(workspace_name, &workspace_window_ids, &window_ids);

        // Build layout context and compute layouts
        let context = self.build_layout_context(workspace_name, &screen_id)?;
        let layout_windows = self.build_layout_windows(&window_ids, &actual_windows);
        let layouts = self.compute_layouts(&layout_mode, &layout_windows, &context)?;

        // Apply the computed layouts
        Self::apply_window_layouts(&layouts);

        Ok(())
    }

    /// Gets the app IDs assigned to a workspace from rules.
    fn get_workspace_app_ids(&self, workspace_name: &str) -> Vec<String> {
        self.config
            .workspaces
            .iter()
            .find(|ws| ws.name == workspace_name)
            .map(|ws| ws.rules.iter().filter_map(|rule| rule.app_id.clone()).collect())
            .unwrap_or_default()
    }

    /// Counts windows per app for multi-window detection.
    fn count_windows_per_app(
        actual_windows: &[ManagedWindow],
        workspace_app_ids: &[String],
    ) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for win in actual_windows {
            if let Some(ref bundle_id) = win.bundle_id
                && workspace_app_ids
                    .iter()
                    .any(|app_id| bundle_id.contains(app_id) || bundle_id == app_id)
            {
                *counts.entry(bundle_id.clone()).or_insert(0) += 1;
            }
        }
        counts
    }

    /// Filters window IDs to only include valid windows for the workspace.
    ///
    /// Valid windows must:
    /// 1. Still exist on the system
    /// 2. Either belong to apps assigned by rules OR be explicitly added
    /// 3. Not be small helper/splash windows for multi-window apps
    /// 4. Not be `PiP` windows (for non-floating layouts)
    #[allow(clippy::too_many_arguments)]
    fn filter_valid_window_ids(
        &self,
        workspace_window_ids: &[u64],
        actual_windows: &[ManagedWindow],
        actual_window_ids: &HashSet<u64>,
        workspace_app_ids: &[String],
        app_window_counts: &HashMap<String, usize>,
        workspace_name: &str,
        layout_mode: &LayoutMode,
    ) -> Vec<u64> {
        workspace_window_ids
            .iter()
            .copied()
            .filter(|id| {
                self.is_valid_layout_window(
                    *id,
                    actual_windows,
                    actual_window_ids,
                    workspace_app_ids,
                    app_window_counts,
                    workspace_name,
                    layout_mode,
                )
            })
            .collect()
    }

    /// Checks if a window is valid for inclusion in the layout.
    #[allow(clippy::too_many_arguments)]
    fn is_valid_layout_window(
        &self,
        window_id: u64,
        actual_windows: &[ManagedWindow],
        actual_window_ids: &HashSet<u64>,
        workspace_app_ids: &[String],
        app_window_counts: &HashMap<String, usize>,
        workspace_name: &str,
        layout_mode: &LayoutMode,
    ) -> bool {
        // Must exist in actual windows
        if !actual_window_ids.contains(&window_id) {
            return false;
        }

        let Some(actual_win) = actual_windows.iter().find(|w| w.id == window_id) else {
            return false;
        };

        // Check if explicitly assigned or matches rules
        if !self.window_belongs_to_workspace(actual_win, workspace_app_ids, workspace_name) {
            return false;
        }

        // Filter out small helper/splash windows for multi-window apps
        if Self::is_helper_window(actual_win, app_window_counts) {
            return false;
        }

        // Skip PiP windows for non-floating layouts
        if *layout_mode != LayoutMode::Floating && window::is_pip_window(actual_win) {
            return false;
        }

        true
    }

    /// Checks if a window belongs to a workspace (explicitly assigned or matches rules).
    fn window_belongs_to_workspace(
        &self,
        window: &ManagedWindow,
        workspace_app_ids: &[String],
        workspace_name: &str,
    ) -> bool {
        // Check if explicitly assigned
        let is_explicitly_assigned = self
            .workspace_manager
            .state()
            .get_window(window.id)
            .is_some_and(|w| w.workspace == workspace_name);

        // Check if matches workspace rules
        let matches_rules = window.bundle_id.as_ref().is_some_and(|bundle_id| {
            workspace_app_ids
                .iter()
                .any(|app_id| bundle_id.contains(app_id) || bundle_id == app_id)
        });

        is_explicitly_assigned || matches_rules
    }

    /// Checks if a window is a small helper/splash window that should be filtered out.
    fn is_helper_window(
        window: &ManagedWindow,
        app_window_counts: &HashMap<String, usize>,
    ) -> bool {
        let Some(ref bundle_id) = window.bundle_id else {
            return false;
        };

        let window_count = app_window_counts.get(bundle_id).copied().unwrap_or(0);
        if window_count <= 1 {
            return false;
        }

        // Only filter windows that are small (likely helper/splash windows)
        // Main windows (larger than 600x400) are always kept
        let is_small_window = window.frame.width < 600 || window.frame.height < 400;
        if !is_small_window {
            return false;
        }

        // Skip small windows with empty titles
        if window.title.is_empty() {
            return true;
        }

        // Skip small windows where title is just the app name (likely splash/placeholder)
        window.title == window.app_name
    }

    /// Removes stale windows from the workspace and state.
    fn cleanup_stale_windows(
        &mut self,
        workspace_name: &str,
        workspace_window_ids: &[u64],
        valid_window_ids: &[u64],
    ) {
        // Find stale IDs
        let stale_ids: Vec<u64> = workspace_window_ids
            .iter()
            .copied()
            .filter(|id| !valid_window_ids.contains(id))
            .collect();

        // Remove from windows map
        for stale_id in stale_ids {
            self.workspace_manager.state_mut().windows.remove(&stale_id);
        }

        // Update workspace's window list
        let valid_ids = valid_window_ids.to_vec();
        if let Some(ws) = self.workspace_manager.state_mut().get_workspace_mut(workspace_name) {
            ws.windows.retain(|id| valid_ids.contains(id));
        }
    }

    /// Builds the layout context for a workspace.
    fn build_layout_context(
        &self,
        workspace_name: &str,
        screen_id: &str,
    ) -> Result<layout::LayoutContext, TilingError> {
        let screen_count = self.workspace_manager.state().screens.len();
        let screen = self
            .workspace_manager
            .state()
            .get_screen(screen_id)
            .ok_or_else(|| TilingError::ScreenNotFound(screen_id.to_string()))?
            .clone();

        let split_ratios = self
            .workspace_manager
            .state()
            .get_workspace(workspace_name)
            .map(|ws| ws.split_ratios.clone())
            .unwrap_or_default();

        let gaps = ResolvedGaps::from_config(&self.config.gaps, &screen, screen_count);

        Ok(layout::LayoutContext {
            screen_frame: screen.usable_frame,
            gaps,
            split_ratios,
        })
    }

    /// Builds layout window objects from window IDs.
    fn build_layout_windows(
        &self,
        window_ids: &[u64],
        actual_windows: &[ManagedWindow],
    ) -> Vec<layout::LayoutWindow> {
        window_ids
            .iter()
            .filter_map(|&id| {
                let managed_win = self.workspace_manager.state().get_window(id)?;
                // Verify window still exists in actual windows
                actual_windows.iter().find(|w| w.id == id)?;
                Some(layout::LayoutWindow {
                    id: managed_win.id,
                    is_floating: managed_win.is_floating,
                    is_minimized: managed_win.is_minimized,
                    is_fullscreen: managed_win.is_fullscreen,
                })
            })
            .collect()
    }

    /// Computes window layouts based on the layout mode.
    fn compute_layouts(
        &self,
        layout_mode: &LayoutMode,
        layout_windows: &[layout::LayoutWindow],
        context: &layout::LayoutContext,
    ) -> Result<Vec<layout::WindowLayout>, TilingError> {
        match layout_mode {
            LayoutMode::Monocle => {
                let monocle = layout::MonocleLayout::new();
                monocle.layout(layout_windows, context)
            }
            // TODO: Implement scrolling layout (currently falls back to tiling)
            LayoutMode::Tiling | LayoutMode::Scrolling => {
                let tiling = layout::TilingLayout::new();
                tiling.layout(layout_windows, context)
            }
            LayoutMode::Split => {
                let split = layout::SplitLayout::auto();
                split.layout(layout_windows, context)
            }
            LayoutMode::SplitVertical => {
                let split = layout::SplitLayout::vertical();
                split.layout(layout_windows, context)
            }
            LayoutMode::SplitHorizontal => {
                let split = layout::SplitLayout::horizontal();
                split.layout(layout_windows, context)
            }
            LayoutMode::Master => {
                let master = layout::MasterLayout::new(self.config.master.clone());
                master.layout(layout_windows, context)
            }
            LayoutMode::Floating => {
                let floating = layout::FloatingLayout::new();
                floating.layout(layout_windows, context)
            }
        }
    }

    /// Applies computed layouts to windows (with animation if enabled).
    fn apply_window_layouts(layouts: &[layout::WindowLayout]) {
        if crate::tiling::animation::is_enabled() {
            let targets: Vec<(u64, crate::tiling::state::WindowFrame)> =
                layouts.iter().map(|wl| (wl.id, wl.frame)).collect();
            crate::tiling::animation::animate_windows(targets);
        } else {
            super::mark_layout_applied();
            for window_layout in layouts {
                if let Err(e) = window::set_window_frame(window_layout.id, &window_layout.frame) {
                    eprintln!("Failed to set window frame for {}: {}", window_layout.id, e);
                }
            }
        }
    }

    /// Sets the layout for a workspace and re-applies it.
    pub fn set_workspace_layout(
        &mut self,
        workspace_name: &str,
        layout_mode: LayoutMode,
    ) -> Result<(), TilingError> {
        // Update the workspace's layout
        {
            let workspace = self
                .workspace_manager
                .state_mut()
                .get_workspace_mut(workspace_name)
                .ok_or_else(|| TilingError::WorkspaceNotFound(workspace_name.to_string()))?;

            // When switching FROM Floating to any other layout, reset is_floating on all windows
            // in this workspace so they participate in the new layout
            let was_floating = workspace.layout == LayoutMode::Floating;
            let is_now_floating = layout_mode == LayoutMode::Floating;

            workspace.layout = layout_mode;
            // Reset split ratios when layout changes
            workspace.split_ratios.clear();

            // Get window IDs for the workspace
            let window_ids: Vec<u64> = workspace.windows.clone();

            // If switching from Floating to a tiled layout, reset is_floating flags
            if was_floating && !is_now_floating {
                for window_id in window_ids {
                    if let Some(win) =
                        self.workspace_manager.state_mut().windows.get_mut(&window_id)
                    {
                        win.is_floating = false;
                    }
                }
            }
        }

        // Emit workspaces changed event for layout change
        self.emit_workspaces_changed();

        // Re-apply the layout
        self.apply_layout(workspace_name)
    }
}
