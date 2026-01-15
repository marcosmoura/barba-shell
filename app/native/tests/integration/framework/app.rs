//! Application management for integration tests.

use std::thread;
use std::time::{Duration, Instant};

use super::{Frame, Window, native};

/// Default timeout for operations.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Poll interval for checking state.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// How long frames must remain unchanged to be considered stable.
const STABILITY_DURATION: Duration = Duration::from_millis(500);

/// Represents an application that can be used in tests.
///
/// When created via `Test::app()`, the app is automatically prepared:
/// 1. Launched if not running
/// 2. All existing windows are closed (clears restored session state)
/// 3. App is quit
///
/// This ensures `create_window()` starts from a clean state.
pub struct App {
    /// The name of the application (e.g., "Dictionary", "TextEdit").
    name: String,
    /// Number of windows created through this App instance.
    window_count: usize,
    /// Whether the app has been prepared (windows cleared).
    prepared: bool,
}

impl App {
    /// Creates a new App wrapper for the given application name.
    ///
    /// This does NOT prepare the app - call `prepare()` to clear existing windows.
    pub(crate) fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            window_count: 0,
            prepared: false,
        }
    }

    /// Creates a new App wrapper and prepares it for testing.
    ///
    /// This will:
    /// 1. Launch the app if not running
    /// 2. Close all existing windows (clears restored session state)
    /// 3. Quit the app
    ///
    /// After this, `create_window()` will start from a clean state.
    pub(crate) fn new_and_prepare(name: &str) -> Self {
        let mut app = Self::new(name);
        app.prepare();
        app
    }

    /// Prepares the app for testing by clearing any existing windows.
    ///
    /// This will:
    /// 1. Launch the app if not running
    /// 2. Wait for session restore to complete
    /// 3. Close all existing windows (app quits automatically when last window closes)
    /// 4. Wait for app to quit
    ///
    /// This ensures subsequent `create_window()` calls start fresh.
    pub fn prepare(&mut self) {
        if self.prepared {
            return;
        }

        eprintln!("Preparing app '{}' for testing...", self.name);

        // Launch the app if not running
        if !self.is_running() {
            eprintln!("  Launching {}...", self.name);
            native::launch_app(&self.name);
            // Poll until app is running
            if !self.wait_for_running(Duration::from_secs(5)) {
                panic!("Failed to launch {} within timeout", self.name);
            }
        }

        // Poll until window count stabilizes (session restore completes)
        eprintln!("  Waiting for session restore to complete...");
        self.wait_for_windows_stable(Duration::from_secs(3));

        // Close all existing windows (app will quit when last window closes)
        self.close_all_windows();

        // Poll until app quits (it quits automatically when last window closes)
        eprintln!("  Waiting for {} to quit...", self.name);
        if !self.wait_for_quit(Duration::from_secs(5)) {
            // Force kill if it doesn't quit
            eprintln!("  Force killing {}...", self.name);
            let _ = std::process::Command::new("pkill").args(["-9", "-x", &self.name]).output();
            self.wait_for_quit(Duration::from_secs(2));
        }

        eprintln!("  {} is prepared (clean state)", self.name);
        self.prepared = true;
    }

    /// Waits for the window count to stabilize (stop changing).
    fn wait_for_windows_stable(&self, timeout: Duration) {
        let start = Instant::now();
        let stability_duration = Duration::from_millis(500);

        let mut last_count = self.get_window_count_internal();
        let mut stable_since = Instant::now();

        while start.elapsed() < timeout {
            thread::sleep(POLL_INTERVAL);

            let current_count = self.get_window_count_internal();

            if current_count == last_count {
                // Count is stable
                if stable_since.elapsed() >= stability_duration {
                    return;
                }
            } else {
                // Count changed, reset stability timer
                last_count = current_count;
                stable_since = Instant::now();
            }
        }
    }

    /// Closes all windows and polls until they're closed.
    fn close_all_windows(&self) {
        let windows = native::get_app_windows(&self.name);
        let window_count = windows.len();

        if window_count == 0 {
            return;
        }

        eprintln!("  Closing {} existing window(s)...", window_count);
        for window in windows {
            native::close_window(window);
            native::release_window(window);
        }

        // Poll for windows to close
        self.wait_for_no_windows(Duration::from_secs(3));
    }

    /// Polls until the app has no windows.
    fn wait_for_no_windows(&self, timeout: Duration) {
        let start = Instant::now();

        while start.elapsed() < timeout {
            let count = self.get_window_count_internal();
            if count == 0 {
                eprintln!("  All windows closed");
                return;
            }
            thread::sleep(POLL_INTERVAL);
        }

        eprintln!(
            "  Warning: {} window(s) may still be open",
            self.get_window_count_internal()
        );
    }

    /// Gets window count without creating Window objects (internal use).
    fn get_window_count_internal(&self) -> usize {
        let windows = native::get_app_windows(&self.name);
        let count = windows.len();
        // Release the AX refs
        for w in windows {
            native::release_window(w);
        }
        count
    }

    /// Returns the application name.
    pub fn name(&self) -> &str { &self.name }

    /// Checks if this application is currently running.
    pub fn is_running(&self) -> bool { native::get_app_pid(&self.name).is_some() }

    /// Waits for the application to start running.
    fn wait_for_running(&self, timeout: Duration) -> bool {
        let start = Instant::now();

        while start.elapsed() < timeout {
            if self.is_running() {
                return true;
            }
            thread::sleep(POLL_INTERVAL);
        }

        false
    }

    /// Creates a new window for this application.
    ///
    /// This method:
    /// 1. Launches the app if not running
    /// 2. Creates a new window/document
    /// 3. Waits for the window to be ready
    /// 4. Returns a Window object for interaction
    ///
    /// # Panics
    ///
    /// Panics if the window cannot be created within the timeout.
    pub fn create_window(&mut self) -> Window { self.create_window_with_timeout(DEFAULT_TIMEOUT) }

    /// Creates a new window with a custom timeout.
    pub fn create_window_with_timeout(&mut self, timeout: Duration) -> Window {
        const MAX_RETRIES: usize = 3;

        for attempt in 0..MAX_RETRIES {
            // Get current window count
            let initial_count = self.get_window_count_internal();

            // Create the window based on app type
            let created = match self.name.as_str() {
                "Dictionary" => native::create_dictionary_window(),
                "TextEdit" => native::create_textedit_window(),
                _ => panic!("Unsupported app for window creation: {}", self.name),
            };

            if !created {
                if attempt < MAX_RETRIES - 1 {
                    eprintln!(
                        "  Retry {}/{}: create {} window failed, retrying...",
                        attempt + 1,
                        MAX_RETRIES,
                        self.name
                    );
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                panic!(
                    "Failed to create {} window after {} retries",
                    self.name, MAX_RETRIES
                );
            }

            // Wait for a new window to appear (count increases) - shorter timeout for retries
            let wait_timeout = if attempt == MAX_RETRIES - 1 {
                timeout
            } else {
                Duration::from_secs(3)
            };

            match self.try_wait_for_new_window(initial_count, wait_timeout) {
                Some(window) => {
                    self.window_count += 1;
                    return window;
                }
                None => {
                    if attempt < MAX_RETRIES - 1 {
                        eprintln!(
                            "  Retry {}/{}: waiting for {} window timed out, retrying...",
                            attempt + 1,
                            MAX_RETRIES,
                            self.name
                        );
                        thread::sleep(Duration::from_millis(500));
                        continue;
                    }
                }
            }
        }

        panic!(
            "Timed out waiting for new {} window after {} retries",
            self.name, MAX_RETRIES
        );
    }

    /// Waits for a new window to appear and returns it.
    ///
    /// Polls until window count exceeds initial_count, then returns the newest window.
    fn wait_for_new_window(&self, initial_count: usize, timeout: Duration) -> Window {
        let start = Instant::now();
        let target_count = initial_count + 1;

        // Poll until we have more windows than we started with
        while start.elapsed() < timeout {
            let windows = native::get_app_windows(&self.name);
            let count = windows.len();

            if count >= target_count {
                // Found new window(s) - return the last one (most recently created)
                // Release all but the last window
                for (i, w) in windows.iter().enumerate() {
                    if i < count - 1 {
                        native::release_window(*w);
                    }
                }

                let new_ref = windows[count - 1];
                let window = Window::new(new_ref, &self.name, self.window_count);

                // Wait for this window's frame to stabilize before returning
                let _ = window.stable_frame();

                return window;
            }

            // Release all windows
            for w in windows {
                native::release_window(w);
            }

            thread::sleep(POLL_INTERVAL);
        }

        panic!(
            "Timed out waiting for new {} window (waited {:?}, have {}, need {})",
            self.name,
            timeout,
            self.get_window_count_internal(),
            target_count
        );
    }

    /// Tries to wait for a new window, returns None on timeout instead of panicking.
    fn try_wait_for_new_window(&self, initial_count: usize, timeout: Duration) -> Option<Window> {
        let start = Instant::now();
        let target_count = initial_count + 1;

        while start.elapsed() < timeout {
            let windows = native::get_app_windows(&self.name);
            let count = windows.len();

            if count >= target_count {
                for (i, w) in windows.iter().enumerate() {
                    if i < count - 1 {
                        native::release_window(*w);
                    }
                }

                let new_ref = windows[count - 1];
                let window = Window::new(new_ref, &self.name, self.window_count);
                let _ = window.stable_frame();

                return Some(window);
            }

            for w in windows {
                native::release_window(w);
            }

            thread::sleep(POLL_INTERVAL);
        }

        None
    }

    /// Gets all current windows for this application (fresh references).
    ///
    /// Returns newly created Window objects with fresh AX references.
    /// Use this when you need to interact with windows (e.g., close them).
    pub fn get_windows(&self) -> Vec<Window> {
        let ax_windows = native::get_app_windows(&self.name);
        ax_windows
            .into_iter()
            .enumerate()
            .map(|(i, w)| Window::new(w, &self.name, i))
            .collect()
    }

    /// Gets frames for all windows (instant, no waiting).
    ///
    /// Returns current frames using fresh AX references.
    pub fn get_frames(&self) -> Vec<Frame> {
        let windows = native::get_app_windows(&self.name);
        let frames: Vec<_> = windows.iter().filter_map(|w| native::get_window_frame(*w)).collect();

        // Release all refs
        for w in windows {
            native::release_window(w);
        }

        frames
    }

    /// Gets frames for all windows after waiting for them to stabilize.
    ///
    /// Polls until:
    /// 1. We have at least `min_count` windows
    /// 2. All frames have been stable for `STABILITY_DURATION`
    ///
    /// This is the recommended way to get frames after creating windows,
    /// as it ensures tiling has completed.
    pub fn get_stable_frames(&self, min_count: usize) -> Vec<Frame> {
        self.get_stable_frames_with_timeout(min_count, DEFAULT_TIMEOUT)
    }

    /// Gets stable frames with a custom timeout.
    pub fn get_stable_frames_with_timeout(
        &self,
        min_count: usize,
        timeout: Duration,
    ) -> Vec<Frame> {
        self.get_stable_frames_internal(min_count, timeout, true)
    }

    /// Gets stable frames for stacked layouts (like monocle) where windows overlap.
    ///
    /// Unlike `get_stable_frames`, this doesn't require frames to be unique,
    /// which is necessary for layouts where all windows have the same position.
    pub fn get_stable_frames_stacked(&self, min_count: usize) -> Vec<Frame> {
        self.get_stable_frames_internal(min_count, DEFAULT_TIMEOUT, false)
    }

    /// Internal implementation for stable frame retrieval.
    fn get_stable_frames_internal(
        &self,
        min_count: usize,
        timeout: Duration,
        require_unique: bool,
    ) -> Vec<Frame> {
        let start = Instant::now();

        let mut last_frames: Vec<Frame> = Vec::new();
        let mut stable_since: Option<Instant> = None;

        while start.elapsed() < timeout {
            let current_frames = self.get_frames();

            // Check if we have enough windows
            let has_enough = current_frames.len() >= min_count;

            // Check if frames match previous poll
            let frames_match = current_frames.len() == last_frames.len()
                && current_frames.iter().zip(last_frames.iter()).all(|(a, b)| a == b);

            // Check that all frames are unique (no duplicates)
            // Duplicates indicate windows are still being repositioned
            // Skip this check for stacked layouts like monocle
            let uniqueness_ok = if require_unique {
                let mut seen = std::collections::HashSet::new();
                current_frames.iter().all(|f| seen.insert((f.x, f.y, f.width, f.height)))
            } else {
                true
            };

            if has_enough && frames_match && uniqueness_ok {
                if let Some(since) = stable_since {
                    if since.elapsed() >= STABILITY_DURATION {
                        return current_frames;
                    }
                } else {
                    stable_since = Some(Instant::now());
                }
            } else {
                last_frames = current_frames;
                stable_since = None;
            }

            thread::sleep(POLL_INTERVAL);
        }

        eprintln!(
            "Warning: get_stable_frames timed out (have {} frames, wanted {})",
            last_frames.len(),
            min_count
        );
        last_frames
    }

    /// Quits this application gracefully.
    pub fn quit(&self) -> bool { native::quit_app(&self.name) }

    /// Waits for the application to quit.
    pub fn wait_for_quit(&self, timeout: Duration) -> bool {
        let start = Instant::now();

        while start.elapsed() < timeout {
            if !self.is_running() {
                return true;
            }
            thread::sleep(POLL_INTERVAL);
        }

        false
    }

    /// Cleans up this application by closing all windows and quitting.
    ///
    /// This is called automatically during test cleanup.
    pub fn cleanup(&self) {
        if !self.is_running() {
            return;
        }

        eprintln!("  Cleaning up {}...", self.name);

        // First, close all windows
        let window_count = self.get_window_count_internal();
        if window_count > 0 {
            eprintln!("    Closing {} window(s)...", window_count);
            self.close_all_windows();
        }

        // Then quit the app (it may have already quit when last window closed)
        if self.is_running() {
            eprintln!("    Quitting {}...", self.name);
            self.quit();

            // Wait for graceful quit
            if !self.wait_for_quit(Duration::from_secs(2)) {
                eprintln!("    {} didn't quit gracefully", self.name);
            }
        }
    }
}
