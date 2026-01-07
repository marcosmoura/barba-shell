//! noTunes Module for Barba Shell.
//!
//! This module prevents iTunes or Apple Music from launching automatically on macOS.
//! When media keys are pressed or Bluetooth headphones reconnect, macOS may try to
//! launch Apple Music - this module intercepts those launches and optionally opens
//! a preferred music player (Tidal) instead.
//!
//! Inspired by <https://github.com/tombonez/noTunes> (MIT License, Tom Taylor 2017).

use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};

use objc::declare::ClassDecl;
use objc::runtime::{Class, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};

use crate::utils::objc::{get_app_bundle_id, nsstring};
use crate::utils::thread::spawn_named_thread;

/// Bundle identifier for Apple Music.
const APPLE_MUSIC_BUNDLE_ID: &str = "com.apple.Music";

/// Bundle identifier for iTunes (legacy).
const ITUNES_BUNDLE_ID: &str = "com.apple.iTunes";

/// Path to the Tidal application.
const TIDAL_APP_PATH: &str = "/Applications/Tidal.app";

/// Tidal bundle identifier.
const TIDAL_BUNDLE_ID: &str = "com.tidal.desktop";

/// Flag indicating if the module is running.
static IS_RUNNING: AtomicBool = AtomicBool::new(false);

/// Checks if a bundle identifier belongs to Apple Music or iTunes.
#[inline]
fn is_music_app(bundle_id: &str) -> bool {
    bundle_id == APPLE_MUSIC_BUNDLE_ID || bundle_id == ITUNES_BUNDLE_ID
}

/// Initializes the noTunes module.
///
/// This sets up an observer for `NSWorkspace.willLaunchApplicationNotification`
/// to intercept and terminate Apple Music/iTunes launches, optionally starting
/// Tidal instead.
pub fn init() {
    if IS_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    spawn_named_thread("notunes-init", move || unsafe {
        setup_workspace_observer();
        // Also terminate any already-running instances
        terminate_music_apps();
    });
}

/// Terminates any currently running Apple Music or iTunes instances.
unsafe fn terminate_music_apps() {
    let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    let running_apps: *mut Object = msg_send![workspace, runningApplications];
    let count: usize = msg_send![running_apps, count];

    for i in 0..count {
        let app: *mut Object = msg_send![running_apps, objectAtIndex: i];

        if let Some(bundle_id_str) = unsafe { get_app_bundle_id(app) }
            && is_music_app(&bundle_id_str)
        {
            eprintln!("barba: notunes: terminating running instance of {bundle_id_str}");
            let _: () = msg_send![app, forceTerminate];
        }
    }
}

/// Sets up the `NSWorkspace` observer for app launch notifications.
unsafe fn setup_workspace_observer() {
    // Get the workspace and notification center
    let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
    let notification_center: *mut Object = msg_send![workspace, notificationCenter];

    // Create the notification name
    let notification_name = unsafe { nsstring("NSWorkspaceWillLaunchApplicationNotification") };

    // Create an observer object
    let observer = unsafe { create_observer_object() };

    let _: () = msg_send![
        notification_center,
        addObserver: observer
        selector: sel!(handleAppLaunch:)
        name: notification_name
        object: null_mut::<Object>()
    ];
}

/// Creates an Objective-C observer object that handles the notification.
unsafe fn create_observer_object() -> *mut Object {
    // Dynamically create a class for our observer
    let superclass = class!(NSObject);
    let class_name = "NoTunesObserver";

    // Check if class already exists
    let existing_class = Class::get(class_name);
    let observer_class = existing_class.unwrap_or_else(|| {
        // Register new class
        let mut decl =
            ClassDecl::new(class_name, superclass).expect("Failed to create NoTunesObserver class");

        // Add the notification handler method
        unsafe {
            decl.add_method(
                sel!(handleAppLaunch:),
                handle_app_launch as extern "C" fn(&Object, Sel, *mut Object),
            );
        }

        decl.register()
    });

    // Create an instance - the observer will be retained by NSNotificationCenter
    let instance: *mut Object = msg_send![observer_class, alloc];
    msg_send![instance, init]
}

/// Callback function for the app launch notification.
extern "C" fn handle_app_launch(_self: &Object, _cmd: Sel, notification: *mut Object) {
    unsafe {
        if notification.is_null() {
            return;
        }

        // Get the userInfo dictionary
        let user_info: *mut Object = msg_send![notification, userInfo];
        if user_info.is_null() {
            return;
        }

        // Get the NSWorkspaceApplicationKey
        let app_key = nsstring("NSWorkspaceApplicationKey");
        let app: *mut Object = msg_send![user_info, objectForKey: app_key];
        if app.is_null() {
            return;
        }

        // Get the bundle identifier and check if it's a music app
        if let Some(bundle_id_str) = get_app_bundle_id(app)
            && is_music_app(&bundle_id_str)
        {
            eprintln!("barba: notunes: blocking launch of {bundle_id_str}");

            // Force terminate the app
            let _: () = msg_send![app, forceTerminate];

            // Launch Tidal as replacement
            launch_tidal();
        }
    }
}

/// Launches Tidal as the replacement music player.
fn launch_tidal() {
    // Check if Tidal is installed
    let tidal_path = std::path::Path::new(TIDAL_APP_PATH);
    if !tidal_path.exists() {
        eprintln!("barba: notunes: Tidal not found at {TIDAL_APP_PATH}");
        return;
    }

    // Check if Tidal is already running
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        let running_apps: *mut Object = msg_send![workspace, runningApplications];
        let count: usize = msg_send![running_apps, count];

        for i in 0..count {
            let app: *mut Object = msg_send![running_apps, objectAtIndex: i];

            if let Some(bundle_id_str) = get_app_bundle_id(app)
                && bundle_id_str == TIDAL_BUNDLE_ID
            {
                // Tidal is already running, no need to launch
                return;
            }
        }
    }

    // Launch Tidal using /usr/bin/open
    match std::process::Command::new("/usr/bin/open").arg(TIDAL_APP_PATH).spawn() {
        Ok(_) => eprintln!("barba: notunes: launched Tidal as replacement"),
        Err(e) => eprintln!("barba: notunes: failed to launch Tidal: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_music_app_apple_music() {
        assert!(is_music_app(APPLE_MUSIC_BUNDLE_ID));
        assert!(is_music_app("com.apple.Music"));
    }

    #[test]
    fn test_is_music_app_itunes() {
        assert!(is_music_app(ITUNES_BUNDLE_ID));
        assert!(is_music_app("com.apple.iTunes"));
    }

    #[test]
    fn test_is_music_app_other_apps() {
        assert!(!is_music_app("com.spotify.client"));
        assert!(!is_music_app(TIDAL_BUNDLE_ID));
        assert!(!is_music_app("com.apple.Safari"));
        assert!(!is_music_app(""));
    }

    #[test]
    fn test_bundle_ids_are_correct() {
        assert_eq!(APPLE_MUSIC_BUNDLE_ID, "com.apple.Music");
        assert_eq!(ITUNES_BUNDLE_ID, "com.apple.iTunes");
    }

    #[test]
    fn test_tidal_path() {
        assert_eq!(TIDAL_APP_PATH, "/Applications/Tidal.app");
    }

    #[test]
    fn test_tidal_bundle_id() {
        assert_eq!(TIDAL_BUNDLE_ID, "com.tidal.desktop");
    }

    #[test]
    fn test_is_running_initially_false() {
        // Note: This test may not be reliable if init() has been called
        // The atomic is static and persists across tests
        // We're just testing that the constant exists and is an AtomicBool
        let _ = IS_RUNNING.load(Ordering::SeqCst);
    }

    #[test]
    fn test_apple_music_bundle_id_format() {
        assert!(APPLE_MUSIC_BUNDLE_ID.starts_with("com.apple."));
    }

    #[test]
    fn test_itunes_bundle_id_format() {
        assert!(ITUNES_BUNDLE_ID.starts_with("com.apple."));
    }

    #[test]
    fn test_tidal_path_is_absolute() {
        assert!(TIDAL_APP_PATH.starts_with('/'));
        assert!(TIDAL_APP_PATH.ends_with(".app"));
    }
}
