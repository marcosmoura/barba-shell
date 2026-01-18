//! Core infrastructure for Stache.
//!
//! This module provides foundational types and utilities used throughout the application:
//!
//! - [`error`] - Unified error types
//! - [`events`] - Tauri event definitions
//! - [`constants`] - Application constants
//! - [`prelude`] - Common re-exports for convenience

pub mod constants;
pub mod error;
pub mod events;
pub mod prelude;

pub use error::{Error, Result, StacheError};
pub use events::EventEmitter;
