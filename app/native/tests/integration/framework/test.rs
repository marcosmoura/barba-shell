//! Main Test struct for integration tests.
//!
//! Provides a clean API for setting up and tearing down test environments.

use std::collections::HashMap;
use std::time::Duration;

use super::stache::StacheProcess;
use super::{App, Frame, Screen, Window};

/// Main test orchestrator.
///
/// Creates a clean test environment with:
/// - Stache running with a specific fixture config
/// - Access to screen information
/// - App management for creating test windows
///
/// Automatically cleans up on drop.
///
/// # Example
///
/// ```rust,ignore
/// let mut test = Test::new("tiling_basic");
/// let screen = test.main_screen();
///
/// // Get an App object - this prepares the app (clears existing windows)
/// let dictionary = test.app("Dictionary");
///
/// // Create windows from a clean state
/// let window1 = dictionary.create_window();
/// let window2 = dictionary.create_window();
///
/// // Test assertions...
/// let frame = window1.frame();
///
/// // Cleanup is automatic on drop, or call explicitly
/// test.cleanup();
/// ```
pub struct Test {
    /// The stache process.
    stache: StacheProcess,
    /// Cached apps by name.
    apps: HashMap<String, App>,
    /// Order in which apps were registered (for cleanup).
    app_order: Vec<String>,
    /// The fixture name used.
    fixture_name: String,
    /// Whether cleanup has already been performed.
    cleaned_up: bool,
}

impl Test {
    /// Creates a new test environment with the given fixture config.
    ///
    /// This will:
    /// 1. Kill any existing stache processes
    /// 2. Start stache with the fixture config
    /// 3. Wait for stache to be fully initialized
    ///
    /// # Panics
    ///
    /// Panics if stache fails to start or initialize within the timeout.
    pub fn new(fixture_name: &str) -> Self {
        Self::with_timeout(fixture_name, Duration::from_secs(30))
    }

    /// Creates a new test environment with a custom timeout.
    pub fn with_timeout(fixture_name: &str, timeout: Duration) -> Self {
        let stache = StacheProcess::start_with_timeout(fixture_name, timeout);

        Self {
            stache,
            apps: HashMap::new(),
            app_order: Vec::new(),
            fixture_name: fixture_name.to_string(),
            cleaned_up: false,
        }
    }

    /// Gets the main screen.
    pub fn main_screen(&self) -> Screen { Screen::main() }

    /// Gets all active screens.
    pub fn all_screens(&self) -> Vec<Screen> { Screen::all() }

    /// Finds the screen containing the given frame (based on center point).
    pub fn screen_containing(&self, frame: &Frame) -> Option<Screen> {
        Screen::containing_frame(frame)
    }

    /// Gets an App instance for interacting with the named application.
    ///
    /// On first call for an app, this will:
    /// 1. Register the app for cleanup tracking
    /// 2. Prepare the app (launch, close existing windows, quit)
    ///
    /// This ensures subsequent `create_window()` calls start from a clean state.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let dictionary = test.app("Dictionary");
    /// let window = dictionary.create_window(); // Starts fresh, no leftover windows
    /// ```
    pub fn app(&mut self, name: &str) -> &mut App {
        if !self.apps.contains_key(name) {
            // Register app for cleanup tracking
            self.app_order.push(name.to_string());

            // Create and prepare the app (clears existing windows)
            let app = App::new_and_prepare(name);
            self.apps.insert(name.to_string(), app);
        }
        self.apps.get_mut(name).unwrap()
    }

    /// Creates a window for the named app and returns it.
    ///
    /// This is a convenience method that:
    /// 1. Gets (and prepares if needed) the App
    /// 2. Creates a new window
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let window = test.create_window("Dictionary");
    /// ```
    pub fn create_window(&mut self, app_name: &str) -> Window {
        // Ensure app is registered and prepared
        if !self.apps.contains_key(app_name) {
            self.app_order.push(app_name.to_string());
            let app = App::new_and_prepare(app_name);
            self.apps.insert(app_name.to_string(), app);
        }
        let app = self.apps.get_mut(app_name).unwrap();
        app.create_window()
    }

    /// Returns the fixture name used for this test.
    pub fn fixture_name(&self) -> &str { &self.fixture_name }

    /// Checks if stache is ready.
    pub fn is_stache_ready(&self) -> bool { self.stache.is_ready() }

    /// Cleans up the test environment.
    ///
    /// This will:
    /// 1. Kill the stache process (so it doesn't interfere with cleanup)
    /// 2. Close all windows for each registered app
    /// 3. Quit each registered app
    /// 4. Wait until apps are fully terminated
    ///
    /// This is called automatically on drop, but can be called
    /// explicitly for more control.
    pub fn cleanup(&mut self) {
        if self.cleaned_up {
            return;
        }
        self.cleaned_up = true;

        eprintln!("Cleaning up test environment...");

        // Kill stache first so it doesn't try to manage closing windows
        self.stache.kill();

        // Close all windows and quit each registered app
        for app_name in &self.app_order {
            if let Some(app) = self.apps.get(app_name) {
                app.cleanup();
            }
        }

        // Force kill any remaining processes and wait for them to terminate
        for app_name in &self.app_order {
            let _ = std::process::Command::new("pkill").args(["-9", "-x", app_name]).output();
        }

        // Wait until all registered apps are really gone
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            let any_running = self.app_order.iter().any(|app_name| {
                std::process::Command::new("pgrep")
                    .args(["-x", app_name])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            });

            if !any_running {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        eprintln!("Cleanup complete.");
    }
}

impl Drop for Test {
    fn drop(&mut self) { self.cleanup(); }
}
