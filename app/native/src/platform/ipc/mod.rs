//! Inter-Process Communication for Stache.
//!
//! This module provides IPC mechanisms for communication between the CLI and desktop app:
//!
//! - [`notification`] - `NSDistributedNotificationCenter` for fire-and-forget notifications
//! - [`socket`] - Unix domain socket for request-response queries

pub mod notification;
pub mod socket;

// Re-export commonly used types
pub use notification::{
    NotificationHandler, StacheNotification, register_notification_handler, send_notification,
    start_notification_listener,
};
pub use socket::{
    IpcError, IpcQuery, IpcResponse, is_app_running, send_query, start_server, stop_server,
};
