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

    /// Returns the number of connected screens.
    pub fn screen_count(&self) -> usize { Screen::all().len() }

    /// Returns true if there are multiple screens connected.
    pub fn has_multiple_screens(&self) -> bool { self.screen_count() >= 2 }

    /// Gets the secondary screen (non-main screen).
    ///
    /// Returns `None` if there's only one screen or no secondary screen can be found.
    pub fn secondary_screen(&self) -> Option<Screen> {
        let screens = Screen::all();
        if screens.len() < 2 {
            return None;
        }
        screens.into_iter().find(|s| !s.is_main())
    }

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

    /// Gets frames for all windows of the named app (instant, no waiting).
    pub fn get_app_frames(&self, app_name: &str) -> Vec<Frame> {
        self.apps.get(app_name).map_or_else(Vec::new, App::get_frames)
    }

    /// Gets stable frames for the named app.
    pub fn get_app_stable_frames(&self, app_name: &str, min_count: usize) -> Vec<Frame> {
        self.apps
            .get(app_name)
            .map_or_else(Vec::new, |app| app.get_stable_frames(min_count))
    }

    /// Gets stable frames for the named app (stacked - for monocle layout).
    pub fn get_app_stable_frames_stacked(&self, app_name: &str, min_count: usize) -> Vec<Frame> {
        self.apps
            .get(app_name)
            .map_or_else(Vec::new, |app| app.get_stable_frames_stacked(min_count))
    }

    /// Gets all frames across all registered apps (instant, no waiting).
    pub fn get_all_frames(&self) -> Vec<Frame> {
        self.apps.values().flat_map(App::get_frames).collect()
    }

    /// Waits for all windows across all registered apps to stabilize.
    ///
    /// This is the recommended way to get frames in multi-app tests, as it
    /// ensures ALL windows have stabilized together (same tiling state).
    ///
    /// Returns a HashMap of app_name -> Vec<Frame>.
    pub fn get_all_stable_frames(
        &self,
        expected_counts: &[(&str, usize)],
    ) -> HashMap<String, Vec<Frame>> {
        self.get_all_stable_frames_internal(expected_counts, Duration::from_secs(10), true)
    }

    /// Gets all stable frames for stacked layouts (like monocle).
    pub fn get_all_stable_frames_stacked(
        &self,
        expected_counts: &[(&str, usize)],
    ) -> HashMap<String, Vec<Frame>> {
        self.get_all_stable_frames_internal(expected_counts, Duration::from_secs(10), false)
    }

    /// Internal implementation for getting stable frames across all apps.
    fn get_all_stable_frames_internal(
        &self,
        expected_counts: &[(&str, usize)],
        timeout: Duration,
        require_unique: bool,
    ) -> HashMap<String, Vec<Frame>> {
        use std::time::Instant;

        const POLL_INTERVAL: Duration = Duration::from_millis(100);
        // Longer stability duration for multi-app scenarios where the tiling manager
        // may take longer to settle after rapid window creation/destruction
        const STABILITY_DURATION: Duration = Duration::from_millis(1000);

        // Initial delay to let the tiling manager process any pending events
        // and clean up stale window references from app preparation
        std::thread::sleep(Duration::from_millis(500));

        let start = Instant::now();
        let mut last_all_frames: Vec<Frame> = Vec::new();
        let mut stable_since: Option<Instant> = None;

        while start.elapsed() < timeout {
            // Get frames for all registered apps
            let mut current_by_app: HashMap<String, Vec<Frame>> = HashMap::new();
            for (name, app) in &self.apps {
                current_by_app.insert(name.clone(), app.get_frames());
            }

            // Check expected counts - at least the expected number
            let mut counts_ok = true;
            for (app_name, expected) in expected_counts {
                let actual = current_by_app.get(*app_name).map_or(0, Vec::len);
                if actual < *expected {
                    counts_ok = false;
                    break;
                }
            }

            // Flatten all frames for comparison
            let current_all_frames: Vec<Frame> =
                current_by_app.values().flatten().cloned().collect();

            // Check if frames match previous poll
            let frames_match = current_all_frames.len() == last_all_frames.len()
                && current_all_frames.iter().zip(last_all_frames.iter()).all(|(a, b)| a == b);

            // Check uniqueness if required
            let uniqueness_ok = if require_unique {
                let mut seen = std::collections::HashSet::new();
                current_all_frames.iter().all(|f| seen.insert((f.x, f.y, f.width, f.height)))
            } else {
                true
            };

            if counts_ok && frames_match && uniqueness_ok {
                if let Some(since) = stable_since {
                    if since.elapsed() >= STABILITY_DURATION {
                        return current_by_app;
                    }
                } else {
                    stable_since = Some(Instant::now());
                }
            } else {
                last_all_frames = current_all_frames;
                stable_since = None;
            }

            std::thread::sleep(POLL_INTERVAL);
        }

        // Timeout - return current state
        eprintln!(
            "Warning: get_all_stable_frames timed out (have {} total frames)",
            last_all_frames.len()
        );

        let mut result: HashMap<String, Vec<Frame>> = HashMap::new();
        for (name, app) in &self.apps {
            result.insert(name.clone(), app.get_frames());
        }
        result
    }

    /// Returns the fixture name used for this test.
    pub fn fixture_name(&self) -> &str { &self.fixture_name }

    /// Runs a stache CLI command and returns the output.
    ///
    /// This is used for testing tiling operations like focus, swap, resize, etc.
    /// The command is run against the currently running stache instance.
    ///
    /// # Arguments
    ///
    /// * `args` - Command arguments (without "stache" prefix)
    ///
    /// # Returns
    ///
    /// * `Some(String)` - stdout if command succeeded
    /// * `None` - if command failed or timed out
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// test.stache_command(&["tiling", "window", "--focus", "next"]);
    /// test.stache_command(&["tiling", "workspace", "--focus", "code"]);
    /// test.stache_command(&["tiling", "window", "--swap", "left"]);
    /// ```
    pub fn stache_command(&self, args: &[&str]) -> Option<String> {
        use std::process::Command;

        let binary_path = StacheProcess::binary_path();

        let output = Command::new(&binary_path).args(args).output().ok()?;

        if output.status.success() {
            Some(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("stache command failed: {:?} {:?}", args, stderr);
            None
        }
    }

    /// Runs a stache CLI command with a delay after.
    ///
    /// Convenience method that adds a short delay to let the operation complete.
    pub fn stache_command_with_delay(&self, args: &[&str], delay_ms: u64) -> Option<String> {
        let result = self.stache_command(args);
        std::thread::sleep(Duration::from_millis(delay_ms));
        result
    }

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
