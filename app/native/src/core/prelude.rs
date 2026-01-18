//! Common re-exports for convenience.
//!
//! This module provides a prelude that can be imported to get access to
//! commonly used types and traits throughout the application.
//!
//! # Usage
//!
//! ```ignore
//! use crate::core::prelude::*;
//! ```

pub use super::constants::{APP_NAME, APP_VERSION, BUNDLE_ID};
pub use super::error::{Error, Result, StacheError};
pub use super::events::{self, EventEmitter};
