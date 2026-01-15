//! Window representation for integration tests.

use std::ffi::c_void;
use std::thread;
use std::time::{Duration, Instant};

use super::native;

/// Default timeout for stability polling.
const DEFAULT_STABILITY_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a frame must remain unchanged to be considered stable.
const STABILITY_DURATION: Duration = Duration::from_millis(500);

/// Poll interval for stability checks.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Window frame with position and size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Frame {
    /// Creates a new Frame with the given position and size.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x: x as i32,
            y: y as i32,
            width: width as i32,
            height: height as i32,
        }
    }

    /// Returns the right edge X coordinate.
    pub fn right(&self) -> i32 { self.x + self.width }

    /// Returns the bottom edge Y coordinate.
    pub fn bottom(&self) -> i32 { self.y + self.height }

    /// Returns the area of this frame.
    pub fn area(&self) -> f64 { (self.width as f64) * (self.height as f64) }

    /// Checks if this frame is approximately equal to another within a tolerance.
    ///
    /// Tolerance is applied to position (x, y) and size (width, height) independently.
    pub fn approximately_equals(&self, other: &Frame, tolerance: i32) -> bool {
        (self.x - other.x).abs() <= tolerance
            && (self.y - other.y).abs() <= tolerance
            && (self.width - other.width).abs() <= tolerance
            && (self.height - other.height).abs() <= tolerance
    }

    /// Alias for `approximately_equals` to match old API naming.
    pub fn approx_eq(&self, other: &Frame, tolerance: i32) -> bool {
        self.approximately_equals(other, tolerance)
    }
}

impl std::fmt::Display for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Frame {{ x: {}, y: {}, width: {}, height: {} }}",
            self.x, self.y, self.width, self.height
        )
    }
}

/// Represents a window managed by the test framework.
///
/// Windows are automatically tracked and can be closed on cleanup.
pub struct Window {
    /// The AXUIElementRef for this window
    ax_element: *mut c_void,
    /// The application name this window belongs to
    app_name: String,
    /// Window index within the app (for identification)
    index: usize,
    /// Whether this window has been closed
    closed: bool,
}

// Window contains a raw pointer but we manage its lifecycle carefully
unsafe impl Send for Window {}
unsafe impl Sync for Window {}

impl Window {
    /// Creates a new Window wrapper.
    pub(crate) fn new(ax_element: *mut c_void, app_name: &str, index: usize) -> Self {
        Self {
            ax_element,
            app_name: app_name.to_string(),
            index,
            closed: false,
        }
    }

    /// Gets the current frame of this window (instant, no waiting).
    ///
    /// Returns `None` if the window no longer exists or cannot be queried.
    pub fn frame(&self) -> Option<Frame> {
        if self.closed {
            return None;
        }
        native::get_window_frame(self.ax_element)
    }

    /// Gets the frame after waiting for it to stabilize.
    ///
    /// Polls until the frame hasn't changed for `STABILITY_DURATION`,
    /// or until timeout is reached.
    pub fn stable_frame(&self) -> Option<Frame> {
        self.stable_frame_with_timeout(DEFAULT_STABILITY_TIMEOUT)
    }

    /// Gets the frame after waiting for it to stabilize, with custom timeout.
    pub fn stable_frame_with_timeout(&self, timeout: Duration) -> Option<Frame> {
        if self.closed {
            return None;
        }

        let start = Instant::now();
        let mut last_frame: Option<Frame> = None;
        let mut stable_since: Option<Instant> = None;

        while start.elapsed() < timeout {
            let current = self.frame();

            match (&last_frame, &current) {
                (Some(last), Some(curr)) if last == curr => {
                    // Frame is stable
                    if let Some(since) = stable_since {
                        if since.elapsed() >= STABILITY_DURATION {
                            return current;
                        }
                    } else {
                        stable_since = Some(Instant::now());
                    }
                }
                _ => {
                    // Frame changed or just started
                    last_frame = current;
                    stable_since = None;
                }
            }

            thread::sleep(POLL_INTERVAL);
        }

        // Return last known frame even if not fully stable
        last_frame
    }

    /// Closes this window.
    pub fn close(&mut self) -> bool {
        if self.closed {
            return true;
        }
        let result = native::close_window(self.ax_element);
        if result {
            self.closed = true;
        }
        result
    }

    /// Returns the app name this window belongs to.
    pub fn app_name(&self) -> &str { &self.app_name }

    /// Returns the window index within its app.
    pub fn index(&self) -> usize { self.index }
}

impl Drop for Window {
    fn drop(&mut self) {
        if !self.ax_element.is_null() {
            native::release_window(self.ax_element);
        }
    }
}

impl std::fmt::Debug for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Window")
            .field("app_name", &self.app_name)
            .field("index", &self.index)
            .field("closed", &self.closed)
            .field("frame", &self.frame())
            .finish()
    }
}
