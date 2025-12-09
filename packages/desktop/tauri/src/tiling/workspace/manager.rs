//! Workspace manager.
//!
//! This module handles workspace creation, switching, and window assignment.

use barba_shared::TilingConfig;

use crate::tiling::error::TilingError;
use crate::tiling::screen;
#[cfg(test)]
use crate::tiling::state::{ManagedWindow, WindowFrame};
use crate::tiling::state::{TilingState, Workspace};

/// Result type for workspace operations.
pub type WorkspaceResult<T> = Result<T, TilingError>;

/// Backup of state needed for screen reinitialization.
struct ReinitBackup {
    focused_workspace: Option<String>,
    focused_per_screen: std::collections::HashMap<String, String>,
    windows: std::collections::HashMap<u64, crate::tiling::state::ManagedWindow>,
    fallback_workspaces: std::collections::HashMap<String, barba_shared::ScreenTarget>,
}

/// Manages workspaces and their state.
pub struct WorkspaceManager {
    /// The current tiling state.
    state: TilingState,

    /// Configuration.
    config: TilingConfig,
}

impl WorkspaceManager {
    /// Creates a new workspace manager.
    #[must_use]
    pub fn new(config: TilingConfig) -> Self {
        Self {
            state: TilingState::new(),
            config,
        }
    }

    /// Initializes the workspace manager, discovering screens and creating default workspaces.
    pub fn initialize(&mut self) -> WorkspaceResult<()> {
        // Discover screens
        self.state.screens = screen::get_all_screens()?;

        // Collect screen IDs first to avoid borrow conflicts
        let screen_ids: Vec<String> = self.state.screens.iter().map(|s| s.id.clone()).collect();

        // Create workspaces for each screen based on config
        for screen_id in &screen_ids {
            self.create_workspaces_for_screen(screen_id);
        }

        // Focus the first workspace on EACH screen
        for screen_id in &screen_ids {
            if let Some(first_ws) = self.state.get_workspaces_on_screen(screen_id).first() {
                let ws_name = first_ws.name.clone();
                self.state.focused_workspace_per_screen.insert(screen_id.clone(), ws_name);
            }
        }

        // Set the global focused workspace to the first workspace on the main screen
        if let Some(main_screen) = self.state.get_main_screen() {
            let screen_id = main_screen.id.clone();
            if let Some(first_ws) = self.state.get_workspaces_on_screen(&screen_id).first() {
                self.state.focused_workspace = Some(first_ws.name.clone());
            }
        }

        Ok(())
    }

    /// Reinitializes screens after a display configuration change.
    ///
    /// This:
    /// 1. Rediscovers all connected screens
    /// 2. Reassigns workspaces to the correct screens (with fallback if needed)
    /// 3. Migrates workspaces from fallback to their intended screen when available
    /// 4. Migrates windows to appropriate workspaces
    /// 5. Preserves the current workspace focus where possible
    pub fn reinitialize_screens(&mut self) -> WorkspaceResult<()> {
        // Backup current state
        let backup = self.backup_current_state();

        // Rediscover screens and check if they changed
        let new_screens = screen::get_all_screens()?;
        if !self.screens_changed(&new_screens) {
            self.state.screens = new_screens;
            return Ok(());
        }

        self.log_screen_change(new_screens.len());
        self.state.screens = new_screens;

        // Recreate workspaces for the new screen configuration
        let old_layouts = self.backup_workspace_layouts();
        self.recreate_workspaces();
        self.restore_workspace_layouts(&old_layouts);

        // Log workspace migrations
        self.log_workspace_migrations(&backup.fallback_workspaces);

        // Restore windows and focus state
        self.restore_windows(backup.windows);
        self.restore_focus_state(backup.focused_workspace.as_deref(), &backup.focused_per_screen);

        Ok(())
    }

    /// Backs up the current state before reinitialization.
    fn backup_current_state(&self) -> ReinitBackup {
        ReinitBackup {
            focused_workspace: self.state.focused_workspace.clone(),
            focused_per_screen: self.state.focused_workspace_per_screen.clone(),
            windows: self.state.windows.clone(),
            fallback_workspaces: self
                .state
                .workspaces
                .iter()
                .filter_map(|ws| {
                    ws.intended_screen.as_ref().map(|target| (ws.name.clone(), target.clone()))
                })
                .collect(),
        }
    }

    /// Checks if the screen configuration has changed.
    fn screens_changed(&self, new_screens: &[crate::tiling::state::Screen]) -> bool {
        let old_ids: std::collections::HashSet<_> =
            self.state.screens.iter().map(|s| &s.id).collect();
        let new_ids: std::collections::HashSet<_> = new_screens.iter().map(|s| &s.id).collect();
        old_ids != new_ids
    }

    /// Logs screen configuration change.
    fn log_screen_change(&self, new_count: usize) {
        eprintln!(
            "barba: screen configuration changed ({} -> {} screens)",
            self.state.screens.len(),
            new_count
        );
    }

    /// Backs up workspace layouts before clearing.
    fn backup_workspace_layouts(
        &self,
    ) -> std::collections::HashMap<String, barba_shared::LayoutMode> {
        self.state
            .workspaces
            .iter()
            .map(|ws| (ws.name.clone(), ws.layout.clone()))
            .collect()
    }

    /// Clears and recreates workspaces for the new screen configuration.
    fn recreate_workspaces(&mut self) {
        self.state.workspaces.clear();
        self.state.focused_workspace_per_screen.clear();

        let screen_ids: Vec<String> = self.state.screens.iter().map(|s| s.id.clone()).collect();
        for screen_id in &screen_ids {
            self.create_workspaces_for_screen(screen_id);
        }
    }

    /// Restores layout overrides from old workspaces.
    fn restore_workspace_layouts(
        &mut self,
        old_layouts: &std::collections::HashMap<String, barba_shared::LayoutMode>,
    ) {
        for ws in &mut self.state.workspaces {
            if let Some(old_layout) = old_layouts.get(&ws.name) {
                ws.layout = old_layout.clone();
            }
        }
    }

    /// Logs workspaces that migrated from fallback to their intended screen.
    fn log_workspace_migrations(
        &self,
        fallback_workspaces: &std::collections::HashMap<String, barba_shared::ScreenTarget>,
    ) {
        for ws in &self.state.workspaces {
            if let Some(old_target) = fallback_workspaces.get(&ws.name)
                && ws.intended_screen.is_none()
            {
                eprintln!(
                    "barba: workspace '{}' migrated to intended screen '{}'",
                    ws.name, old_target
                );
            }
        }
    }

    /// Restores windows to their workspaces after reinitialization.
    fn restore_windows(
        &mut self,
        windows_backup: std::collections::HashMap<u64, crate::tiling::state::ManagedWindow>,
    ) {
        for (window_id, mut window) in windows_backup {
            if self.state.get_workspace(&window.workspace).is_some() {
                // Workspace exists, restore window
                self.add_window_to_workspace(window_id, &window.workspace);
                self.state.windows.insert(window_id, window);
            } else {
                // Workspace doesn't exist, assign to main screen's first workspace
                if let Some(ws_name) = self.get_main_screen_fallback_workspace() {
                    window.workspace.clone_from(&ws_name);
                    self.add_window_to_workspace(window_id, &ws_name);
                    self.state.windows.insert(window_id, window);
                }
            }
        }
    }

    /// Adds a window to a workspace if not already present.
    fn add_window_to_workspace(&mut self, window_id: u64, workspace_name: &str) {
        if let Some(ws) = self.state.get_workspace_mut(workspace_name)
            && !ws.windows.contains(&window_id)
        {
            ws.windows.push(window_id);
        }
    }

    /// Gets the first workspace on the main screen as a fallback.
    fn get_main_screen_fallback_workspace(&self) -> Option<String> {
        let main_screen = self.state.get_main_screen()?;
        let main_screen_id = main_screen.id.clone();
        self.state
            .get_workspaces_on_screen(&main_screen_id)
            .first()
            .map(|ws| ws.name.clone())
    }

    /// Restores focus state after reinitialization.
    fn restore_focus_state(
        &mut self,
        old_focused_workspace: Option<&str>,
        old_focused_per_screen: &std::collections::HashMap<String, String>,
    ) {
        // Restore per-screen focus
        let screen_ids: Vec<String> = self.state.screens.iter().map(|s| s.id.clone()).collect();
        for screen_id in &screen_ids {
            let ws_name = self.determine_screen_focus(screen_id, old_focused_per_screen);
            if let Some(name) = ws_name {
                self.state.focused_workspace_per_screen.insert(screen_id.clone(), name);
            }
        }

        // Restore global focused workspace
        self.restore_global_focus(old_focused_workspace);
    }

    /// Determines which workspace should be focused on a screen.
    fn determine_screen_focus(
        &self,
        screen_id: &str,
        old_focused_per_screen: &std::collections::HashMap<String, String>,
    ) -> Option<String> {
        // Try to restore old focus if workspace is still on this screen
        if let Some(old_ws) = old_focused_per_screen.get(screen_id)
            && self.state.get_workspace(old_ws).is_some_and(|ws| ws.screen == *screen_id)
        {
            return Some(old_ws.clone());
        }

        // Fall back to first workspace on this screen
        self.state.get_workspaces_on_screen(screen_id).first().map(|ws| ws.name.clone())
    }

    /// Restores the global focused workspace.
    fn restore_global_focus(&mut self, old_focused_workspace: Option<&str>) {
        if let Some(old_focus) = old_focused_workspace
            && self.state.get_workspace(old_focus).is_some()
        {
            self.state.focused_workspace = Some(old_focus.to_string());
        } else if let Some(ws_name) = self.get_main_screen_fallback_workspace() {
            self.state.focused_workspace = Some(ws_name);
        }
    }

    /// Creates workspaces for a screen based on configuration.
    ///
    /// For each configured workspace:
    /// - If its target screen matches this screen, create it here
    /// - If its target screen doesn't exist AND this is the main screen, create it here (fallback)
    fn create_workspaces_for_screen(&mut self, screen_id: &str) {
        let default_layout = self.config.default_layout.clone();
        let workspace_configs = &self.config.workspaces;

        // If no workspaces configured, create a default one
        if workspace_configs.is_empty() {
            let ws_name = format!("{screen_id}:1");
            self.state.workspaces.push(Workspace::new(
                ws_name,
                default_layout,
                screen_id.to_string(),
            ));
            return;
        }

        let is_main_screen = self.state.get_screen(screen_id).is_some_and(|s| s.is_main);

        // Create workspaces based on config
        for ws_config in workspace_configs {
            use barba_shared::ScreenTarget;

            let screen = self.state.get_screen(screen_id);

            // Determine if this workspace should be created on this screen
            let (should_create, is_fallback) = match &ws_config.screen {
                ScreenTarget::Main => (screen.is_some_and(|s| s.is_main), false),
                ScreenTarget::Secondary => {
                    // Check if secondary screen exists
                    let secondary_exists = self.state.screens.iter().any(|s| !s.is_main);
                    if secondary_exists {
                        // Create on secondary screen only
                        (screen.is_some_and(|s| !s.is_main), false)
                    } else {
                        // Fallback to main if no secondary exists
                        (is_main_screen, true)
                    }
                }
                ScreenTarget::Named(target_name) => {
                    // Check if the named screen exists
                    let named_screen_exists = self
                        .state
                        .screens
                        .iter()
                        .any(|s| s.name == *target_name || s.id == *target_name);

                    if named_screen_exists {
                        // Create only on the matching named screen
                        (
                            screen.is_some_and(|s| s.name == *target_name || s.id == *target_name),
                            false,
                        )
                    } else {
                        // Named screen doesn't exist - fallback to main screen
                        (is_main_screen, true)
                    }
                }
            };

            if should_create {
                if is_fallback {
                    eprintln!(
                        "barba: screen '{}' not found for workspace '{}', falling back to main screen",
                        ws_config.screen, ws_config.name
                    );
                }

                let layout = ws_config.layout.clone();

                // Create workspace, remembering intended screen if in fallback mode
                if is_fallback {
                    self.state.workspaces.push(Workspace::new_with_fallback(
                        ws_config.name.clone(),
                        layout,
                        screen_id.to_string(),
                        ws_config.screen.clone(),
                    ));
                } else {
                    self.state.workspaces.push(Workspace::new(
                        ws_config.name.clone(),
                        layout,
                        screen_id.to_string(),
                    ));
                }
            }
        }
    }

    /// Gets a reference to the current state.
    #[must_use]
    pub const fn state(&self) -> &TilingState { &self.state }

    /// Gets a mutable reference to the current state.
    pub const fn state_mut(&mut self) -> &mut TilingState { &mut self.state }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // WorkspaceManager Creation Tests
    // ========================================================================

    #[test]
    fn test_workspace_manager_new() {
        let config = TilingConfig::default();
        let manager = WorkspaceManager::new(config);
        assert!(manager.state.workspaces.is_empty());
        assert!(manager.state.screens.is_empty());
    }

    // ========================================================================
    // State Access Tests
    // ========================================================================

    #[test]
    fn test_state_access() {
        let manager = WorkspaceManager::new(TilingConfig::default());

        // Test immutable access
        let state = manager.state();
        assert!(state.workspaces.is_empty());
    }

    #[test]
    fn test_state_mut_access() {
        let mut manager = WorkspaceManager::new(TilingConfig::default());

        // Test mutable access
        manager.state_mut().workspaces.push(Workspace::new(
            "test".to_string(),
            barba_shared::LayoutMode::Tiling,
            "1".to_string(),
        ));

        assert_eq!(manager.state().workspaces.len(), 1);
    }

    #[test]
    fn test_workspace_fallback_to_main_screen_for_missing_named_screen() {
        use barba_shared::{ScreenTarget, WorkspaceConfig};

        use crate::tiling::state::{Screen, ScreenFrame};

        // Create config with a workspace targeting a non-existent screen
        let config = TilingConfig {
            workspaces: vec![
                WorkspaceConfig {
                    name: "main-ws".to_string(),
                    screen: ScreenTarget::Main,
                    layout: barba_shared::LayoutMode::Tiling,
                    rules: vec![],
                    preset_on_open: None,
                },
                WorkspaceConfig {
                    name: "missing-screen-ws".to_string(),
                    screen: ScreenTarget::Named("NonExistentScreen".to_string()),
                    layout: barba_shared::LayoutMode::Master,
                    rules: vec![],
                    preset_on_open: None,
                },
            ],
            ..TilingConfig::default()
        };

        let mut manager = WorkspaceManager::new(config);

        // Add a single main screen
        manager.state.screens.push(Screen {
            id: "main-screen-id".to_string(),
            name: "Built-in Display".to_string(),
            frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            usable_frame: ScreenFrame {
                x: 0,
                y: 25,
                width: 1920,
                height: 1055,
            },
            is_main: true,
        });

        // Create workspaces for the main screen
        manager.create_workspaces_for_screen("main-screen-id");

        // Both workspaces should be created on the main screen
        assert_eq!(manager.state.workspaces.len(), 2);

        // Verify main-ws is on main screen
        let main_ws = manager.state.get_workspace("main-ws").unwrap();
        assert_eq!(main_ws.screen, "main-screen-id");

        // Verify missing-screen-ws falls back to main screen
        let fallback_ws = manager.state.get_workspace("missing-screen-ws").unwrap();
        assert_eq!(fallback_ws.screen, "main-screen-id");
        assert_eq!(fallback_ws.layout, barba_shared::LayoutMode::Master);
    }

    #[test]
    fn test_workspace_secondary_fallback_to_main_when_only_one_screen() {
        use barba_shared::{ScreenTarget, WorkspaceConfig};

        use crate::tiling::state::{Screen, ScreenFrame};

        // Create config with a workspace targeting secondary screen
        let config = TilingConfig {
            workspaces: vec![WorkspaceConfig {
                name: "secondary-ws".to_string(),
                screen: ScreenTarget::Secondary,
                layout: barba_shared::LayoutMode::Tiling,
                rules: vec![],
                preset_on_open: None,
            }],
            ..TilingConfig::default()
        };

        let mut manager = WorkspaceManager::new(config);

        // Add only a main screen (no secondary)
        manager.state.screens.push(Screen {
            id: "main-screen-id".to_string(),
            name: "Built-in Display".to_string(),
            frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            usable_frame: ScreenFrame {
                x: 0,
                y: 25,
                width: 1920,
                height: 1055,
            },
            is_main: true,
        });

        // Create workspaces for the main screen
        manager.create_workspaces_for_screen("main-screen-id");

        // Workspace should fall back to main screen
        assert_eq!(manager.state.workspaces.len(), 1);
        let ws = manager.state.get_workspace("secondary-ws").unwrap();
        assert_eq!(ws.screen, "main-screen-id");
    }

    #[test]
    fn test_focus_restoration_after_workspace_migration() {
        use barba_shared::{ScreenTarget, WorkspaceConfig};

        use crate::tiling::state::{Screen, ScreenFrame};

        // Create config with a workspace targeting secondary screen
        let config = TilingConfig {
            workspaces: vec![
                WorkspaceConfig {
                    name: "main-ws".to_string(),
                    screen: ScreenTarget::Main,
                    layout: barba_shared::LayoutMode::Tiling,
                    rules: vec![],
                    preset_on_open: None,
                },
                WorkspaceConfig {
                    name: "secondary-ws".to_string(),
                    screen: ScreenTarget::Secondary,
                    layout: barba_shared::LayoutMode::Tiling,
                    rules: vec![],
                    preset_on_open: None,
                },
            ],
            ..TilingConfig::default()
        };

        let mut manager = WorkspaceManager::new(config);

        // Start with only main screen - secondary-ws falls back to main
        manager.state.screens.push(Screen {
            id: "main-screen-id".to_string(),
            name: "Built-in Display".to_string(),
            frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            usable_frame: ScreenFrame {
                x: 0,
                y: 25,
                width: 1920,
                height: 1055,
            },
            is_main: true,
        });

        manager.create_workspaces_for_screen("main-screen-id");

        // Both workspaces on main, set secondary-ws as focused on main screen
        manager
            .state
            .focused_workspace_per_screen
            .insert("main-screen-id".to_string(), "secondary-ws".to_string());
        manager.state.focused_workspace = Some("secondary-ws".to_string());

        // Add windows to secondary-ws
        for i in 1..=3 {
            manager.state.windows.insert(i, ManagedWindow {
                id: i,
                title: format!("Window {i}"),
                app_name: "App".to_string(),
                bundle_id: None,
                class: None,
                pid: 1,
                workspace: "secondary-ws".to_string(),
                is_floating: false,
                is_minimized: false,
                is_fullscreen: false,
                is_hidden: false,
                frame: WindowFrame::default(),
            });
            if let Some(ws) = manager.state.get_workspace_mut("secondary-ws") {
                ws.windows.push(i);
            }
        }

        // Now simulate adding secondary screen
        manager.state.screens.push(Screen {
            id: "secondary-screen-id".to_string(),
            name: "External Display".to_string(),
            frame: ScreenFrame {
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
            },
            usable_frame: ScreenFrame {
                x: 1920,
                y: 25,
                width: 2560,
                height: 1415,
            },
            is_main: false,
        });

        // Simulate what reinitialize_screens does (but manually since we can't call
        // screen::get_all_screens in tests)
        let old_focused_per_screen = manager.state.focused_workspace_per_screen.clone();

        // Clear and recreate workspaces
        manager.state.workspaces.clear();
        manager.state.focused_workspace_per_screen.clear();

        manager.create_workspaces_for_screen("main-screen-id");
        manager.create_workspaces_for_screen("secondary-screen-id");

        // Verify secondary-ws migrated to secondary screen
        let sec_ws = manager.state.get_workspace("secondary-ws").unwrap();
        assert_eq!(sec_ws.screen, "secondary-screen-id");

        // Now restore focus using the same logic as reinitialize_screens
        let screen_ids = vec![
            "main-screen-id".to_string(),
            "secondary-screen-id".to_string(),
        ];
        for screen_id in &screen_ids {
            let ws_name = if let Some(old_ws) = old_focused_per_screen.get(screen_id)
                && manager.state.get_workspace(old_ws).is_some_and(|ws| ws.screen == *screen_id)
            {
                old_ws.clone()
            } else if let Some(first_ws) = manager.state.get_workspaces_on_screen(screen_id).first()
            {
                first_ws.name.clone()
            } else {
                continue;
            };
            manager.state.focused_workspace_per_screen.insert(screen_id.clone(), ws_name);
        }

        // Verify: main screen should NOT have secondary-ws focused anymore
        // (because secondary-ws migrated to secondary screen)
        let main_focus = manager.state.focused_workspace_per_screen.get("main-screen-id").unwrap();
        assert_eq!(
            main_focus, "main-ws",
            "Main screen should focus main-ws after migration"
        );

        // Verify: secondary screen should have secondary-ws focused
        let sec_focus =
            manager.state.focused_workspace_per_screen.get("secondary-screen-id").unwrap();
        assert_eq!(
            sec_focus, "secondary-ws",
            "Secondary screen should focus secondary-ws"
        );
    }

    #[test]
    fn test_all_windows_restored_after_workspace_migration() {
        use barba_shared::{ScreenTarget, WorkspaceConfig};

        use crate::tiling::state::{Screen, ScreenFrame};

        // Create config with a workspace targeting secondary screen
        let config = TilingConfig {
            workspaces: vec![WorkspaceConfig {
                name: "secondary-ws".to_string(),
                screen: ScreenTarget::Secondary,
                layout: barba_shared::LayoutMode::Tiling,
                rules: vec![],
                preset_on_open: None,
            }],
            ..TilingConfig::default()
        };

        let mut manager = WorkspaceManager::new(config);

        // Start with only main screen - secondary-ws falls back to main
        manager.state.screens.push(Screen {
            id: "main-screen-id".to_string(),
            name: "Built-in Display".to_string(),
            frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            usable_frame: ScreenFrame {
                x: 0,
                y: 25,
                width: 1920,
                height: 1055,
            },
            is_main: true,
        });

        manager.create_workspaces_for_screen("main-screen-id");

        // Add 5 windows to secondary-ws
        for i in 1..=5 {
            manager.state.windows.insert(i, ManagedWindow {
                id: i,
                title: format!("Window {i}"),
                app_name: "App".to_string(),
                bundle_id: None,
                class: None,
                pid: 1,
                workspace: "secondary-ws".to_string(),
                is_floating: false,
                is_minimized: false,
                is_fullscreen: false,
                is_hidden: false,
                frame: WindowFrame::default(),
            });
            if let Some(ws) = manager.state.get_workspace_mut("secondary-ws") {
                ws.windows.push(i);
            }
        }

        // Verify we have 5 windows in the workspace
        let ws = manager.state.get_workspace("secondary-ws").unwrap();
        assert_eq!(ws.windows.len(), 5);

        // Backup windows before migration (simulating reinitialize_screens)
        let windows_backup = manager.state.windows.clone();

        // Add secondary screen
        manager.state.screens.push(Screen {
            id: "secondary-screen-id".to_string(),
            name: "External Display".to_string(),
            frame: ScreenFrame {
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
            },
            usable_frame: ScreenFrame {
                x: 1920,
                y: 25,
                width: 2560,
                height: 1415,
            },
            is_main: false,
        });

        // Clear and recreate workspaces
        manager.state.workspaces.clear();
        manager.state.windows.clear();

        manager.create_workspaces_for_screen("main-screen-id");
        manager.create_workspaces_for_screen("secondary-screen-id");

        // Restore windows (simulating reinitialize_screens logic)
        for (window_id, window) in windows_backup {
            if manager.state.get_workspace(&window.workspace).is_some() {
                if let Some(ws) = manager.state.get_workspace_mut(&window.workspace) {
                    if !ws.windows.contains(&window_id) {
                        ws.windows.push(window_id);
                    }
                }
                manager.state.windows.insert(window_id, window);
            }
        }

        // Verify workspace migrated to secondary screen
        let ws = manager.state.get_workspace("secondary-ws").unwrap();
        assert_eq!(ws.screen, "secondary-screen-id");

        // CRITICAL: Verify ALL 5 windows were restored to the workspace
        assert_eq!(
            ws.windows.len(),
            5,
            "All 5 windows should be restored to the migrated workspace"
        );

        // Verify all windows are in the state
        assert_eq!(manager.state.windows.len(), 5);

        // Verify each window's workspace field
        for i in 1..=5 {
            let win = manager.state.get_window(i).expect("Window should exist");
            assert_eq!(win.workspace, "secondary-ws");
        }
    }
}
