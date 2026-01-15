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
pub use screen::Screen;
/// Re-export for convenience in tests
pub use suite_guard::check_suite_preconditions;
pub use test::Test;
pub use window::{Frame, Window};
