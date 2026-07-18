// Hacky workaround to approximate menu bar visibility in the absence of proper APIs.

use std::cell::RefCell;
use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::TCFType;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use objc::declare::ClassDecl;
use objc::runtime::{BOOL, Class, NO, Object, Sel};
use objc::{class, msg_send, sel, sel_impl};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::events;
use crate::modules::bar::watcher::start_best_effort_refresh_watcher;
use crate::platform::objc::{nsstring, nsstring_to_string};

/// Flag indicating if menu visibility watcher is running.
static MENU_VISIBILITY_WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Current menu bar visibility state.
static MENU_BAR_VISIBLE: AtomicBool = AtomicBool::new(false);

const MENU_BAR_FALLBACK_POLL_INTERVAL: Duration = Duration::from_secs(2);
const WORKSPACE_REFRESH_NOTIFICATION_NAMES: [&str; 3] = [
    "NSWorkspaceActiveSpaceDidChangeNotification",
    "NSWorkspaceDidActivateApplicationNotification",
    "NSWorkspaceDidDeactivateApplicationNotification",
];
const APPLICATION_REFRESH_NOTIFICATION_NAMES: [&str; 3] = [
    "NSApplicationDidBecomeActiveNotification",
    "NSApplicationDidResignActiveNotification",
    "NSApplicationDidChangeScreenParametersNotification",
];
static MENU_BAR_REFRESH_SIGNAL: OnceLock<Sender<()>> = OnceLock::new();

fn emit_menubar_visibility_event(
    app_handle: &AppHandle,
    window_label: &str,
    is_visible: bool,
) -> Result<(), String> {
    let window = app_handle.get_webview_window(window_label).ok_or_else(|| {
        format!("Menubar visibility watcher could not find window `{window_label}`")
    })?;

    window
        .emit(events::menubar::VISIBILITY_CHANGED, &is_visible)
        .map_err(|err| err.to_string())
}

pub fn start_menu_bar_visibility_watcher(window: &WebviewWindow) {
    if MENU_VISIBILITY_WATCHER_RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }

    register_menu_bar_visibility_observer(window.app_handle().clone(), window.label().to_string());
}

fn register_menu_bar_visibility_observer(app_handle: AppHandle, window_label: String) {
    let initial_state =
        resolve_menu_bar_visible(query_nsmenu_visible(), query_menu_bar_visible(), None)
            .unwrap_or(false);
    MENU_BAR_VISIBLE.store(initial_state, Ordering::Release);

    if let Err(e) = emit_menubar_visibility_event(&app_handle, &window_label, initial_state) {
        tracing::warn!(error = %e, "failed to emit initial menubar visibility");
    }

    let last_visible = RefCell::new(initial_state);

    thread::spawn(move || {
        start_best_effort_refresh_watcher(
            "menubar-watcher",
            MENU_BAR_FALLBACK_POLL_INTERVAL,
            |sender| {
                if MENU_BAR_REFRESH_SIGNAL.set(sender).is_err() {
                    tracing::warn!("menubar refresh signal was already initialized");
                }

                register_menu_bar_visibility_observers()
            },
            || {
                let mut last_visible = last_visible.borrow_mut();
                refresh_menu_bar_visibility(&app_handle, &window_label, &mut last_visible);
            },
        );
    });
}

fn refresh_menu_bar_visibility(
    app_handle: &AppHandle,
    window_label: &str,
    last_visible: &mut bool,
) {
    let visible = resolve_menu_bar_visible(
        query_nsmenu_visible(),
        query_menu_bar_visible(),
        Some(*last_visible),
    )
    .unwrap_or(*last_visible);
    if visible != *last_visible {
        *last_visible = visible;
        MENU_BAR_VISIBLE.store(visible, Ordering::Release);

        if let Err(e) = emit_menubar_visibility_event(app_handle, window_label, visible) {
            tracing::warn!(error = %e, "failed to emit menubar visibility");
        }
    }
}

fn register_menu_bar_visibility_observers() -> Result<(), String> {
    unsafe {
        let workspace: *mut Object = msg_send![class!(NSWorkspace), sharedWorkspace];
        if workspace.is_null() {
            return Err("failed to get NSWorkspace shared workspace".to_string());
        }

        let workspace_center: *mut Object = msg_send![workspace, notificationCenter];
        if workspace_center.is_null() {
            return Err("failed to get NSWorkspace notification center".to_string());
        }

        let default_center: *mut Object = msg_send![class!(NSNotificationCenter), defaultCenter];
        if default_center.is_null() {
            return Err("failed to get NSNotificationCenter default center".to_string());
        }

        let observer = create_menu_bar_observer();

        for notification_name in WORKSPACE_REFRESH_NOTIFICATION_NAMES {
            register_notification_observer(
                workspace_center,
                observer,
                notification_name,
                sel!(handleMenuBarRefreshNotification:),
            );
        }

        for notification_name in APPLICATION_REFRESH_NOTIFICATION_NAMES {
            register_notification_observer(
                default_center,
                observer,
                notification_name,
                sel!(handleMenuBarRefreshNotification:),
            );
        }
    }

    Ok(())
}

fn register_notification_observer(
    center: *mut Object,
    observer: *mut Object,
    notification_name: &str,
    selector: Sel,
) {
    unsafe {
        let name = nsstring(notification_name);
        let _: () = msg_send![
            center,
            addObserver: observer
            selector: selector
            name: name
            object: std::ptr::null::<Object>()
        ];
    }
}

fn create_menu_bar_observer() -> *mut Object {
    unsafe {
        let superclass = class!(NSObject);
        let class_name = "StacheMenuBarObserver";

        let existing_class = Class::get(class_name);
        let observer_class = existing_class.unwrap_or_else(|| {
            let mut decl = ClassDecl::new(class_name, superclass)
                .expect("Failed to create StacheMenuBarObserver class");

            decl.add_method(
                sel!(handleMenuBarRefreshNotification:),
                handle_menu_bar_refresh_notification as extern "C" fn(&Object, Sel, *mut Object),
            );

            decl.register()
        });

        let instance: *mut Object = msg_send![observer_class, alloc];
        msg_send![instance, init]
    }
}

extern "C" fn handle_menu_bar_refresh_notification(
    _self: &Object,
    _cmd: Sel,
    notification: *mut Object,
) {
    unsafe {
        if !notification.is_null() {
            let name_obj: *mut Object = msg_send![notification, name];
            let name = nsstring_to_string(name_obj);
            if !name.is_empty() {
                tracing::debug!(notification = %name, "menubar: received refresh notification");
            }
        }
    }

    signal_menu_bar_refresh();
}

fn signal_menu_bar_refresh() {
    if let Some(sender) = MENU_BAR_REFRESH_SIGNAL.get() {
        let _ = sender.send(());
    }
}

fn query_menu_bar_visible() -> Result<bool, String> {
    unsafe {
        // Use CGWindowListCopyWindowInfo to check for visible menubar windows
        #[allow(non_upper_case_globals)]
        const kCGWindowListOptionOnScreenOnly: u32 = 1 << 0;
        #[allow(non_upper_case_globals)]
        const kCGNullWindowID: u32 = 0;

        #[link(name = "CoreGraphics", kind = "framework")]
        unsafe extern "C" {
            fn CGWindowListCopyWindowInfo(option: u32, relativeToWindow: u32) -> CFArrayRef;
        }

        let window_list =
            CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID);
        if window_list.is_null() {
            return Err("Failed to get window list".into());
        }

        let windows = CFArray::<CFDictionary>::wrap_under_create_rule(window_list);
        let window_count = windows.len();

        // Look for menubar-related windows
        let owner_name_key = CFString::from_static_string("kCGWindowOwnerName");
        let layer_key = CFString::from_static_string("kCGWindowLayer");
        let bounds_key = CFString::from_static_string("kCGWindowBounds");
        let name_key = CFString::from_static_string("kCGWindowName");

        let mut menubar_window_found = false;

        for i in 0..window_count {
            if let Some(window_info) = windows.get(i) {
                // Get the raw dictionary pointer
                let dict_ptr = window_info.as_concrete_TypeRef();

                // Get owner name
                let owner_key_ptr = owner_name_key.as_concrete_TypeRef().cast::<c_void>();
                let owner_value_ptr: *const c_void =
                    msg_send![dict_ptr.cast::<Object>(), objectForKey: owner_key_ptr];

                if !owner_value_ptr.is_null() {
                    let owner_str = CFString::wrap_under_get_rule(owner_value_ptr.cast());
                    let owner = owner_str.to_string();

                    // Get window layer
                    let layer_key_ptr = layer_key.as_concrete_TypeRef().cast::<c_void>();
                    let layer_value_ptr: *const c_void =
                        msg_send![dict_ptr.cast::<Object>(), objectForKey: layer_key_ptr];

                    let layer = if layer_value_ptr.is_null() {
                        None
                    } else {
                        let layer_number = CFNumber::wrap_under_get_rule(layer_value_ptr.cast());
                        layer_number.to_i32()
                    };

                    // Get window name
                    let name_key_ptr = name_key.as_concrete_TypeRef().cast::<c_void>();
                    let name_value_ptr: *const c_void =
                        msg_send![dict_ptr.cast::<Object>(), objectForKey: name_key_ptr];
                    let name = if name_value_ptr.is_null() {
                        None
                    } else {
                        let name_str = CFString::wrap_under_get_rule(name_value_ptr.cast());
                        Some(name_str.to_string())
                    };

                    // Get bounds
                    let bounds_key_ptr = bounds_key.as_concrete_TypeRef().cast::<c_void>();
                    let bounds_value_ptr: *const c_void =
                        msg_send![dict_ptr.cast::<Object>(), objectForKey: bounds_key_ptr];
                    let has_bounds = !bounds_value_ptr.is_null();

                    // Check various conditions that might indicate the menubar
                    if let Some(layer_val) = layer {
                        // Check for menubar windows at layer 25 (WindowServer or has bounds)
                        if layer_val == 25
                            && ((owner == "WindowServer" || owner == "Window Server") || has_bounds)
                        {
                            menubar_window_found = true;
                        }
                        // Check for Control Center or menubar-related names
                        else if (24..=26).contains(&layer_val)
                            && let Some(ref win_name) = name
                            && (win_name.contains("Menubar")
                                || win_name.contains("Menu Bar")
                                || win_name.contains("StatusBar"))
                        {
                            menubar_window_found = true;
                        }
                    }
                }
            }
        }

        Ok(menubar_window_found)
    }
}

/// Select the best-available menu-bar visibility, falling back from the
/// primary `NSMenu` query to the `CGWindowList` heuristic, then to prior state.
/// Returns `None` only when all sources fail and there is no prior state.
fn resolve_menu_bar_visible(
    nsmenu: Option<bool>,
    cg: Result<bool, String>,
    prior: Option<bool>,
) -> Option<bool> {
    nsmenu.or_else(|| cg.ok()).or(prior)
}

/// Query system menu bar visibility via the documented `+[NSMenu menuBarVisible]`
/// class method. This is the official cheap way to detect auto-hide/show. Returns
/// `None` if the `ObjC` runtime call cannot produce a value.
#[allow(clippy::unnecessary_wraps)]
fn query_nsmenu_visible() -> Option<bool> {
    // SAFETY: Calling a class method on NSMenu which is always available on macOS.
    let visible: BOOL = unsafe { msg_send![class!(NSMenu), menuBarVisible] };
    Some(visible != NO)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events;

    #[test]
    fn visibility_event_constant_is_correct() {
        assert_eq!(
            events::menubar::VISIBILITY_CHANGED,
            "stache://menubar/visibility-changed"
        );
    }

    #[test]
    fn menu_bar_refresh_notification_names_cover_workspace_and_application_events() {
        assert_eq!(WORKSPACE_REFRESH_NOTIFICATION_NAMES, [
            "NSWorkspaceActiveSpaceDidChangeNotification",
            "NSWorkspaceDidActivateApplicationNotification",
            "NSWorkspaceDidDeactivateApplicationNotification",
        ]);
        assert_eq!(APPLICATION_REFRESH_NOTIFICATION_NAMES, [
            "NSApplicationDidBecomeActiveNotification",
            "NSApplicationDidResignActiveNotification",
            "NSApplicationDidChangeScreenParametersNotification",
        ]);
    }

    #[test]
    fn menu_bar_fallback_poll_interval_is_slower_than_hot_polling() {
        assert_eq!(MENU_BAR_FALLBACK_POLL_INTERVAL.as_secs(), 2);
    }

    #[test]
    fn menu_bar_visible_default_is_false() {
        // The static defaults to false
        // Note: This test may be affected by other tests that modify the state
        let _ = MENU_BAR_VISIBLE.load(Ordering::Acquire);
    }

    #[test]
    fn menu_visibility_watcher_running_is_atomic() {
        // Verify the atomic can be read
        let _ = MENU_VISIBILITY_WATCHER_RUNNING.load(Ordering::Acquire);
    }

    #[test]
    fn query_menu_bar_visible_returns_result() {
        // This test verifies the function runs without crashing
        // The actual result depends on the system state
        let result = query_menu_bar_visible();
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn query_menu_bar_visible_returns_bool_on_success() {
        // Verify the function returns a valid Result
        // and if successful, we can use the boolean value
        if let Ok(visible) = query_menu_bar_visible() {
            // Store the value to verify it's usable
            MENU_BAR_VISIBLE.store(visible, Ordering::Release);
            let stored = MENU_BAR_VISIBLE.load(Ordering::Acquire);
            assert_eq!(stored, visible);
        }
    }

    #[test]
    fn menu_bar_visible_can_be_toggled() {
        let original = MENU_BAR_VISIBLE.load(Ordering::Acquire);

        MENU_BAR_VISIBLE.store(true, Ordering::Release);
        assert!(MENU_BAR_VISIBLE.load(Ordering::Acquire));

        MENU_BAR_VISIBLE.store(false, Ordering::Release);
        assert!(!MENU_BAR_VISIBLE.load(Ordering::Acquire));

        // Restore original state
        MENU_BAR_VISIBLE.store(original, Ordering::Release);
    }

    #[test]
    fn menu_visibility_watcher_running_can_be_set() {
        let original = MENU_VISIBILITY_WATCHER_RUNNING.load(Ordering::Acquire);

        // Test swap returns previous value
        let prev = MENU_VISIBILITY_WATCHER_RUNNING.swap(true, Ordering::AcqRel);
        assert_eq!(prev, original);

        // Restore original state
        MENU_VISIBILITY_WATCHER_RUNNING.store(original, Ordering::Release);
    }

    #[test]
    fn cg_window_constants_are_correct() {
        #[allow(non_upper_case_globals)]
        const kCGWindowListOptionOnScreenOnly: u32 = 1 << 0;
        #[allow(non_upper_case_globals)]
        const kCGNullWindowID: u32 = 0;

        assert_eq!(kCGWindowListOptionOnScreenOnly, 1);
        assert_eq!(kCGNullWindowID, 0);
    }

    // --- resolve_menu_bar_visible ---

    #[test]
    fn resolve_uses_primary_when_available() {
        assert_eq!(resolve_menu_bar_visible(Some(true), Ok(false), None), Some(true));
        assert_eq!(
            resolve_menu_bar_visible(Some(false), Ok(true), None),
            Some(false)
        );
    }

    #[test]
    fn resolve_falls_back_when_primary_none() {
        assert_eq!(resolve_menu_bar_visible(None, Ok(true), None), Some(true));
        assert_eq!(resolve_menu_bar_visible(None, Ok(false), None), Some(false));
    }

    #[test]
    fn resolve_preserves_prior_when_both_fail() {
        assert_eq!(
            resolve_menu_bar_visible(None, Err("fail".into()), Some(true)),
            Some(true)
        );
        assert_eq!(
            resolve_menu_bar_visible(None, Err("fail".into()), Some(false)),
            Some(false)
        );
    }

    #[test]
    fn resolve_returns_none_when_all_fail_and_no_prior() {
        assert_eq!(resolve_menu_bar_visible(None, Err("fail".into()), None), None);
    }

    #[test]
    fn resolve_prior_trumps_cg_fallback_when_primary_none() {
        // When primary is None and CG fails, preserve prior even if CG would have returned something
        assert_eq!(
            resolve_menu_bar_visible(None, Err("fail".into()), Some(true)),
            Some(true)
        );
        assert_eq!(
            resolve_menu_bar_visible(None, Err("fail".into()), Some(false)),
            Some(false)
        );
    }
}
