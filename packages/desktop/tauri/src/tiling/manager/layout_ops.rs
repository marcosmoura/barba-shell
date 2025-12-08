//! Layout application and management.
//!
//! This module handles applying layouts to workspaces, including
//! computing window positions and sizes.

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
        let actual_window_ids: std::collections::HashSet<u64> =
            actual_windows.iter().map(|w| w.id).collect();

        let workspace = self
            .workspace_manager
            .state()
            .get_workspace(workspace_name)
            .ok_or_else(|| TilingError::WorkspaceNotFound(workspace_name.to_string()))?;

        let screen_id = workspace.screen.clone();
        let layout_mode = workspace.layout.clone();

        // Get the apps assigned to this workspace from rules
        let workspace_app_ids: Vec<String> = self
            .config
            .workspaces
            .iter()
            .find(|ws| ws.name == workspace_name)
            .map(|ws| ws.rules.iter().filter_map(|rule| rule.app_id.clone()).collect())
            .unwrap_or_default();

        // Count windows per app to detect multi-window apps
        let mut app_window_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for win in &actual_windows {
            if let Some(ref bundle_id) = win.bundle_id
                && workspace_app_ids
                    .iter()
                    .any(|app_id| bundle_id.contains(app_id) || bundle_id == app_id)
            {
                *app_window_counts.entry(bundle_id.clone()).or_insert(0) += 1;
            }
        }

        // Filter window_ids to only include windows that:
        // 1. Still exist on the system
        // 2. Either: belong to apps assigned to this workspace by rules, OR were explicitly added
        // 3. For multi-window apps, skip small helper/splash windows
        let window_ids: Vec<u64> = workspace
            .windows
            .iter()
            .copied()
            .filter(|id| {
                // Must exist in actual windows
                if !actual_window_ids.contains(id) {
                    return false;
                }

                if let Some(actual_win) = actual_windows.iter().find(|w| w.id == *id) {
                    // Check if this window was explicitly assigned to this workspace in our state
                    let is_explicitly_assigned = self
                        .workspace_manager
                        .state()
                        .get_window(*id)
                        .is_some_and(|w| w.workspace == workspace_name);

                    // Check if it matches workspace rules
                    let matches_rules = if let Some(ref bundle_id) = actual_win.bundle_id {
                        workspace_app_ids
                            .iter()
                            .any(|app_id| bundle_id.contains(app_id) || bundle_id == app_id)
                    } else {
                        false
                    };

                    // Include if explicitly assigned OR matches rules
                    if !is_explicitly_assigned && !matches_rules {
                        return false;
                    }

                    // For apps with multiple windows, filter out placeholder/helper windows
                    // Only filter small windows with empty/generic titles - keep main windows
                    if let Some(ref bundle_id) = actual_win.bundle_id {
                        let window_count = app_window_counts.get(bundle_id).copied().unwrap_or(0);
                        if window_count > 1 {
                            // Only filter windows that are small (likely helper/splash windows)
                            // Main windows (larger than 600x400) are always kept
                            let is_small_window =
                                actual_win.frame.width < 600 || actual_win.frame.height < 400;

                            if is_small_window {
                                // Skip small windows with empty titles
                                if actual_win.title.is_empty() {
                                    return false;
                                }
                                // Skip small windows where title is just the app name (likely splash/placeholder)
                                if actual_win.title == actual_win.app_name {
                                    return false;
                                }
                            }
                        }
                    }

                    // For non-floating layouts, skip Picture-in-Picture windows
                    // PiP windows should float above tiled windows, not be part of the layout
                    if layout_mode != LayoutMode::Floating && window::is_pip_window(actual_win) {
                        return false;
                    }

                    return true;
                }
                false
            })
            .collect();

        // Clean up stale windows from workspace and state
        let stale_ids: Vec<u64> = workspace
            .windows
            .iter()
            .copied()
            .filter(|id| !window_ids.contains(id))
            .collect();

        for stale_id in stale_ids {
            // Remove from windows map
            self.workspace_manager.state_mut().windows.remove(&stale_id);
        }

        // Update workspace's window list to remove stale entries
        let valid_ids = window_ids.clone();
        if let Some(ws) = self.workspace_manager.state_mut().get_workspace_mut(workspace_name) {
            ws.windows.retain(|id| valid_ids.contains(id));
        }

        // Get screen info and count
        let screen_count = self.workspace_manager.state().screens.len();
        let screen = self
            .workspace_manager
            .state()
            .get_screen(&screen_id)
            .ok_or_else(|| TilingError::ScreenNotFound(screen_id.clone()))?
            .clone();

        // Get split ratios from workspace
        let split_ratios = self
            .workspace_manager
            .state()
            .get_workspace(workspace_name)
            .map(|ws| ws.split_ratios.clone())
            .unwrap_or_default();

        // Build layout context
        let gaps = ResolvedGaps::from_config(&self.config.gaps, &screen, screen_count);
        let context = layout::LayoutContext {
            screen_frame: screen.usable_frame,
            gaps,
            split_ratios,
        };

        // Convert windows to layout windows
        // Create layout windows with extra info for stable sorting
        let layout_windows_with_info: Vec<(layout::LayoutWindow, String, String)> = window_ids
            .iter()
            .filter_map(|&id| {
                let managed_win = self.workspace_manager.state().get_window(id)?;
                let actual_win = actual_windows.iter().find(|w| w.id == id)?;
                Some((
                    layout::LayoutWindow {
                        id: managed_win.id,
                        is_floating: managed_win.is_floating,
                        is_minimized: managed_win.is_minimized,
                        is_fullscreen: managed_win.is_fullscreen,
                    },
                    actual_win.bundle_id.clone().unwrap_or_default(),
                    actual_win.title.clone(),
                ))
            })
            .collect();

        // DON'T sort - preserve the order from workspace.windows which may have been
        // explicitly set by user actions like window swapping
        // The order in workspace.windows is the authoritative layout order

        let layout_windows: Vec<layout::LayoutWindow> = layout_windows_with_info
            .iter()
            .map(|(lw, _, _): &(layout::LayoutWindow, String, String)| lw.clone())
            .collect();

        // Get layout implementation and compute layouts
        let layouts = match layout_mode {
            LayoutMode::Monocle => {
                let monocle = layout::MonocleLayout::new();
                monocle.layout(&layout_windows, &context)?
            }
            LayoutMode::Tiling => {
                let tiling = layout::TilingLayout::new();
                tiling.layout(&layout_windows, &context)?
            }
            LayoutMode::Split => {
                let split = layout::SplitLayout::auto();
                split.layout(&layout_windows, &context)?
            }
            LayoutMode::SplitVertical => {
                let split = layout::SplitLayout::vertical();
                split.layout(&layout_windows, &context)?
            }
            LayoutMode::SplitHorizontal => {
                let split = layout::SplitLayout::horizontal();
                split.layout(&layout_windows, &context)?
            }
            LayoutMode::Master => {
                let master = layout::MasterLayout::new(self.config.master.clone());
                master.layout(&layout_windows, &context)?
            }
            LayoutMode::Floating => {
                // Floating layout: windows keep their current positions
                let floating = layout::FloatingLayout::new();
                floating.layout(&layout_windows, &context)?
            }
            // TODO: Implement scrolling layout
            LayoutMode::Scrolling => {
                // For now, fall back to tiling for scrolling
                let tiling = layout::TilingLayout::new();
                tiling.layout(&layout_windows, &context)?
            }
        };

        // Apply layouts to windows (with animation if enabled)
        // The animation manager handles marking the layout cooldown when animations start.
        if crate::tiling::animation::is_enabled() {
            // Collect all window frames for batch animation
            let targets: Vec<(u64, crate::tiling::state::WindowFrame)> =
                layouts.iter().map(|wl| (wl.id, wl.frame)).collect();
            crate::tiling::animation::animate_windows(targets);
        } else {
            // No animation, apply immediately - mark cooldown manually
            super::mark_layout_applied();
            for window_layout in &layouts {
                if let Err(e) = window::set_window_frame(window_layout.id, &window_layout.frame) {
                    eprintln!("Failed to set window frame for {}: {}", window_layout.id, e);
                }
            }
        }

        Ok(())
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

        // Re-apply the layout
        self.apply_layout(workspace_name)
    }
}
