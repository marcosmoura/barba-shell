//! Service infrastructure for Stache.
//!
//! This module provides traits and utilities for managing application modules
//! and background services.
//!
//! - [`traits`] - Module and service trait definitions
//! - [`thread`] - Thread utilities for spawning named threads and GCD dispatch

pub mod thread;
pub mod traits;

pub use traits::{BackgroundService, Module, ModuleError};
