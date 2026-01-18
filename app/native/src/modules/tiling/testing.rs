//! Mock infrastructure for tiling window manager tests.
//!
//! This module provides mock implementations of screens, windows, and the window
//! manager for testing tiling logic without requiring real macOS windows or
//! accessibility permissions.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::tiling::testing::{MockScreen, MockWindow, MockWindowManager};
//!
//! let mut manager = MockWindowManager::new()
//!     .with_screen(MockScreen::new("main", 1920.0, 1080.0))
//!     .with_screen(MockScreen::new("secondary", 1920.0, 1080.0));
//!
//! manager.add_window(MockWindow::new(1, "Terminal"));
//! manager.add_window(MockWindow::new(2, "Browser"));
//!
//! manager.focus_window(1);
//! assert_eq!(manager.focused_window(), Some(1));
//! ```

#![cfg(test)]

use std::collections::HashMap;

use super::state::Rect;

// ============================================================================
// Mock Screen
// ============================================================================

/// A mock screen for testing.
///
/// Simulates a display with configurable dimensions. By default, creates a
/// screen with a 25px menu bar at the top.
#[derive(Debug, Clone)]
pub struct MockScreen {
    /// Unique screen identifier.
    pub id: u32,
    /// Screen name (e.g., "main", "secondary").
    pub name: String,
    /// Full screen frame.
    pub frame: Rect,
    /// Visible frame (excluding menu bar/dock).
    pub visible_frame: Rect,
    /// Whether this is the main screen.
    pub is_main: bool,
}

impl MockScreen {
    /// Creates a new mock screen with the given name and dimensions.
    ///
    /// The visible frame is automatically calculated with a 25px menu bar.
    #[must_use]
    pub fn new(name: &str, width: f64, height: f64) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let menu_bar_height = 25.0;
        Self {
            id,
            name: name.to_string(),
            frame: Rect::new(0.0, 0.0, width, height),
            visible_frame: Rect::new(0.0, menu_bar_height, width, height - menu_bar_height),
            is_main: id == 1,
        }
    }

    /// Creates a mock screen with specific origin coordinates.
    #[must_use]
    pub fn with_origin(mut self, x: f64, y: f64) -> Self {
        self.frame.x = x;
        self.frame.y = y;
        self.visible_frame.x = x;
        self.visible_frame.y = y + 25.0; // Account for menu bar
        self
    }

    /// Sets whether this is the main screen.
    #[must_use]
    pub fn main(mut self, is_main: bool) -> Self {
        self.is_main = is_main;
        self
    }

    /// Creates a standard 1080p screen.
    #[must_use]
    pub fn hd() -> Self { Self::new("HD", 1920.0, 1080.0) }

    /// Creates a standard 4K screen.
    #[must_use]
    pub fn uhd() -> Self { Self::new("4K", 3840.0, 2160.0) }

    /// Creates a MacBook 14" screen (actual resolution).
    #[must_use]
    pub fn macbook_14() -> Self { Self::new("Built-in", 3024.0, 1964.0) }
}

// ============================================================================
// Mock Window
// ============================================================================

/// A mock window for testing.
///
/// Simulates a window with configurable properties. Windows start with a
/// default 800x600 frame at origin (100, 100).
#[derive(Debug, Clone)]
pub struct MockWindow {
    /// Unique window identifier (CGWindowID).
    pub id: u32,
    /// Window title.
    pub title: String,
    /// Application name.
    pub app_name: String,
    /// Bundle identifier.
    pub bundle_id: Option<String>,
    /// Current window frame.
    pub frame: Rect,
    /// Whether the window is minimized.
    pub minimized: bool,
    /// Whether the window is hidden.
    pub hidden: bool,
    /// Whether the window should be treated as floating.
    pub floating: bool,
}

impl MockWindow {
    /// Creates a new mock window with the given ID and title.
    #[must_use]
    pub fn new(id: u32, title: &str) -> Self {
        Self {
            id,
            title: title.to_string(),
            app_name: title.to_string(),
            bundle_id: None,
            frame: Rect::new(100.0, 100.0, 800.0, 600.0),
            minimized: false,
            hidden: false,
            floating: false,
        }
    }

    /// Sets the application name.
    #[must_use]
    pub fn app(mut self, name: &str) -> Self {
        self.app_name = name.to_string();
        self
    }

    /// Sets the bundle identifier.
    #[must_use]
    pub fn bundle(mut self, bundle_id: &str) -> Self {
        self.bundle_id = Some(bundle_id.to_string());
        self
    }

    /// Sets the window frame.
    #[must_use]
    pub fn frame(mut self, x: f64, y: f64, width: f64, height: f64) -> Self {
        self.frame = Rect::new(x, y, width, height);
        self
    }

    /// Sets the window as floating.
    #[must_use]
    pub fn floating(mut self, floating: bool) -> Self {
        self.floating = floating;
        self
    }

    /// Sets the window as minimized.
    #[must_use]
    pub fn minimized(mut self, minimized: bool) -> Self {
        self.minimized = minimized;
        self
    }
}

// ============================================================================
// Mock Window Manager
// ============================================================================

/// A mock window manager for testing.
///
/// Provides a simplified interface for testing window management logic
/// without requiring real windows or accessibility permissions.
#[derive(Debug, Default)]
pub struct MockWindowManager {
    /// All screens in the system.
    screens: Vec<MockScreen>,
    /// All windows, keyed by window ID.
    windows: HashMap<u32, MockWindow>,
    /// Currently focused window ID.
    focused: Option<u32>,
    /// Window assignment to workspaces: window_id -> workspace_name.
    assignments: HashMap<u32, String>,
    /// Current workspace name.
    current_workspace: String,
}

impl MockWindowManager {
    /// Creates a new empty mock window manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_workspace: "default".to_string(),
            ..Default::default()
        }
    }

    /// Adds a screen to the manager.
    #[must_use]
    pub fn with_screen(mut self, screen: MockScreen) -> Self {
        self.screens.push(screen);
        self
    }

    /// Adds multiple screens to the manager.
    #[must_use]
    pub fn with_screens(mut self, screens: Vec<MockScreen>) -> Self {
        self.screens.extend(screens);
        self
    }

    /// Adds a window to the manager.
    pub fn add_window(&mut self, window: MockWindow) {
        let id = window.id;
        self.windows.insert(id, window);
        self.assignments.insert(id, self.current_workspace.clone());
    }

    /// Removes a window from the manager.
    pub fn remove_window(&mut self, id: u32) -> Option<MockWindow> {
        self.assignments.remove(&id);
        if self.focused == Some(id) {
            self.focused = None;
        }
        self.windows.remove(&id)
    }

    /// Focuses a window by ID.
    pub fn focus_window(&mut self, id: u32) -> bool {
        if self.windows.contains_key(&id) {
            self.focused = Some(id);
            true
        } else {
            false
        }
    }

    /// Returns the currently focused window ID.
    #[must_use]
    pub fn focused_window(&self) -> Option<u32> { self.focused }

    /// Returns a reference to a window by ID.
    #[must_use]
    pub fn get_window(&self, id: u32) -> Option<&MockWindow> { self.windows.get(&id) }

    /// Returns a mutable reference to a window by ID.
    pub fn get_window_mut(&mut self, id: u32) -> Option<&mut MockWindow> {
        self.windows.get_mut(&id)
    }

    /// Moves a window to a new frame.
    pub fn move_window(&mut self, id: u32, frame: Rect) -> bool {
        if let Some(window) = self.windows.get_mut(&id) {
            window.frame = frame;
            true
        } else {
            false
        }
    }

    /// Returns all window IDs.
    #[must_use]
    pub fn window_ids(&self) -> Vec<u32> { self.windows.keys().copied().collect() }

    /// Returns window IDs assigned to a workspace.
    #[must_use]
    pub fn windows_in_workspace(&self, workspace: &str) -> Vec<u32> {
        self.assignments
            .iter()
            .filter(|(_, ws)| *ws == workspace)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Assigns a window to a workspace.
    pub fn assign_to_workspace(&mut self, window_id: u32, workspace: &str) {
        self.assignments.insert(window_id, workspace.to_string());
    }

    /// Sets the current workspace.
    pub fn set_current_workspace(&mut self, workspace: &str) {
        self.current_workspace = workspace.to_string();
    }

    /// Returns the number of screens.
    #[must_use]
    pub fn screen_count(&self) -> usize { self.screens.len() }

    /// Returns a reference to a screen by index.
    #[must_use]
    pub fn get_screen(&self, index: usize) -> Option<&MockScreen> { self.screens.get(index) }

    /// Returns the main screen.
    #[must_use]
    pub fn main_screen(&self) -> Option<&MockScreen> { self.screens.iter().find(|s| s.is_main) }
}

// ============================================================================
// Test Helpers
// ============================================================================

/// Creates a vector of mock windows with sequential IDs.
///
/// Useful for quickly creating multiple windows for layout tests.
#[must_use]
pub fn create_mock_windows(count: usize) -> Vec<MockWindow> {
    (1..=count).map(|i| MockWindow::new(i as u32, &format!("Window {i}"))).collect()
}

/// Creates a vector of TrackedWindow references from mock windows.
///
/// This is useful for testing layout algorithms that expect `&[&TrackedWindow]`.
#[must_use]
pub fn mock_tracked_windows(windows: &[MockWindow]) -> Vec<super::state::TrackedWindow> {
    windows
        .iter()
        .map(|w| super::state::TrackedWindow {
            id: w.id,
            pid: 1000 + w.id as i32,
            app_id: w.bundle_id.clone().unwrap_or_default(),
            app_name: w.app_name.clone(),
            title: w.title.clone(),
            frame: w.frame,
            is_minimized: w.minimized,
            is_hidden: w.hidden,
            is_floating: w.floating,
            workspace_name: "default".to_string(),
        })
        .collect()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_screen_creation() {
        let screen = MockScreen::new("test", 1920.0, 1080.0);
        assert_eq!(screen.name, "test");
        assert_eq!(screen.frame.width, 1920.0);
        assert_eq!(screen.frame.height, 1080.0);
        // Visible frame should have menu bar subtracted
        assert_eq!(screen.visible_frame.y, 25.0);
        assert_eq!(screen.visible_frame.height, 1055.0);
    }

    #[test]
    fn test_mock_screen_with_origin() {
        let screen = MockScreen::new("secondary", 1920.0, 1080.0).with_origin(1920.0, 0.0);
        assert_eq!(screen.frame.x, 1920.0);
        assert_eq!(screen.visible_frame.x, 1920.0);
    }

    #[test]
    fn test_mock_window_creation() {
        let window = MockWindow::new(1, "Test Window")
            .app("Terminal")
            .bundle("com.apple.Terminal")
            .frame(0.0, 0.0, 1000.0, 800.0);

        assert_eq!(window.id, 1);
        assert_eq!(window.title, "Test Window");
        assert_eq!(window.app_name, "Terminal");
        assert_eq!(window.bundle_id, Some("com.apple.Terminal".to_string()));
        assert_eq!(window.frame.width, 1000.0);
    }

    #[test]
    fn test_mock_manager_window_operations() {
        let mut manager = MockWindowManager::new().with_screen(MockScreen::hd());

        manager.add_window(MockWindow::new(1, "Window 1"));
        manager.add_window(MockWindow::new(2, "Window 2"));

        assert_eq!(manager.window_ids().len(), 2);
        assert!(manager.focus_window(1));
        assert_eq!(manager.focused_window(), Some(1));

        manager.remove_window(1);
        assert_eq!(manager.focused_window(), None);
        assert_eq!(manager.window_ids().len(), 1);
    }

    #[test]
    fn test_mock_manager_workspace_assignment() {
        let mut manager = MockWindowManager::new();

        manager.add_window(MockWindow::new(1, "Window 1"));
        manager.set_current_workspace("code");
        manager.add_window(MockWindow::new(2, "Window 2"));

        assert_eq!(manager.windows_in_workspace("default").len(), 1);
        assert_eq!(manager.windows_in_workspace("code").len(), 1);

        manager.assign_to_workspace(1, "code");
        assert_eq!(manager.windows_in_workspace("code").len(), 2);
    }

    #[test]
    fn test_create_mock_windows() {
        let windows = create_mock_windows(5);
        assert_eq!(windows.len(), 5);
        assert_eq!(windows[0].id, 1);
        assert_eq!(windows[4].id, 5);
    }

    #[test]
    fn test_mock_tracked_windows() {
        let mock_windows = vec![
            MockWindow::new(1, "Test").floating(true),
            MockWindow::new(2, "Test 2"),
        ];

        let tracked = mock_tracked_windows(&mock_windows);
        assert_eq!(tracked.len(), 2);
        assert!(tracked[0].is_floating);
        assert!(!tracked[1].is_floating);
    }
}
