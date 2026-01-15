//! Test framework for Stache integration tests.
//!
//! This module provides a clean, ergonomic API for writing integration tests
//! that interact with real macOS windows and the Stache tiling window manager.
//!
//! # Example
//!
//! ```rust,ignore
//! let mut test = Test::new("tiling_basic");
//! let screen = test.main_screen();
//! let dictionary = test.app("Dictionary");
//!
//! let window1 = dictionary.create_window();
//! let window2 = dictionary.create_window();
//!
//! let frame1 = window1.frame();
//! let frame2 = window2.frame();
//!
//! // ... assertions ...
//!
//! test.cleanup();
//! ```

mod app;
mod native;
mod screen;
mod stache;
mod suite_guard;
mod test;
mod window;

pub use app::App;
// Re-export native helpers for operation tests
pub use native::{
    activate_app, get_app_window_count, get_app_window_frames, get_frontmost_app_name,
    get_frontmost_window_frame, get_frontmost_window_title, get_screen_size,
    set_frontmost_window_frame,
};

// =============================================================================
// Constants for operation tests
// =============================================================================

/// Default delay after operations (ms) to let the tiling manager process changes.
pub const OPERATION_DELAY_MS: u64 = 200;

// =============================================================================
// Helper functions
// =============================================================================

/// Delays execution for the specified number of milliseconds.
pub fn delay(ms: u64) { std::thread::sleep(std::time::Duration::from_millis(ms)); }
pub use screen::Screen;
/// Re-export for convenience in tests
pub use suite_guard::check_suite_preconditions;
pub use test::Test;
pub use window::{Frame, Window};
