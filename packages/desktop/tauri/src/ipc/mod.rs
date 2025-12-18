//! IPC module for CLI-to-desktop communication.
//!
//! This module provides a Unix socket server for receiving commands
//! from the standalone CLI application. It is organized into submodules:
//!
//! - `server`: Unix socket server implementation and client handling
//! - `types`: Common types and payloads for IPC communication
//! - `handlers`: Command handlers organized by domain:
//!   - `handlers::wallpaper`: Wallpaper-related commands
//!   - `handlers::system`: System-level commands

mod handlers;
mod server;
mod types;

// Re-export the main entry point
pub use server::start as start_ipc_server;
