//! Screen configuration change watcher.
//!
//! This module monitors for display configuration changes (screens added/removed/changed)
//! and notifies the tiling manager to reinitialize workspaces.

use std::os::raw::c_void;
use std::sync::mpsc::{Sender, channel};

use core_graphics::display::CGDisplayRegisterReconfigurationCallback;

use crate::tiling::manager::try_get_manager;
use crate::utils::thread::spawn_named_thread;

/// Starts watching for screen configuration changes.
///
/// When a display is added, removed, or reconfigured, this will trigger
/// a reinitialize of screens and workspaces in the tiling manager.
pub fn start_screen_watcher() {
    spawn_named_thread("tiling-screen-watcher", move || {
        let (tx, rx) = channel();

        // Register display reconfiguration callback
        unsafe {
            extern "C" fn display_reconfiguration_callback(
                _display: u32,
                _flags: u32,
                user_info: *const c_void,
            ) {
                if !user_info.is_null() {
                    let tx = unsafe { &*user_info.cast::<Sender<()>>() };
                    let _ = tx.send(());
                }
            }

            let tx_ptr: *const Sender<()> = Box::into_raw(Box::new(tx));

            CGDisplayRegisterReconfigurationCallback(
                display_reconfiguration_callback,
                tx_ptr.cast::<c_void>(),
            );
        }

        while rx.recv().is_ok() {
            // Small delay to let the system settle after display changes
            std::thread::sleep(std::time::Duration::from_millis(500));

            // Notify the tiling manager
            if let Some(manager) = try_get_manager() {
                manager.write().handle_screen_change();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_screen_watcher_spawns_thread() {
        // Just verify it doesn't panic when called
        start_screen_watcher();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
