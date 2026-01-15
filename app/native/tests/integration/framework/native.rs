//! Native macOS API helpers for integration tests.
//!
//! Uses Accessibility API, NSWorkspace, and Core Graphics for reliable
//! window/app management. No osascript dependency.

use std::ffi::c_void;
use std::time::Duration;
use std::{ptr, thread};

use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation::base::{CFRelease, CFRetain, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::display::CGDisplay;

use super::Frame;

// =============================================================================
// Accessibility API FFI
// =============================================================================

pub type AXUIElementRef = *mut c_void;
type AXError = i32;

const K_AX_ERROR_SUCCESS: AXError = 0;

// AXValue types for extracting CGPoint/CGSize
const K_AX_VALUE_TYPE_CG_POINT: u32 = 1;
const K_AX_VALUE_TYPE_CG_SIZE: u32 = 2;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut *mut c_void,
    ) -> AXError;
    fn AXUIElementCopyAttributeValues(
        element: AXUIElementRef,
        attribute: CFStringRef,
        index: i64,
        max_values: i64,
        values: *mut CFArrayRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *const c_void,
    ) -> AXError;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> AXError;
    fn AXValueGetValue(value: *const c_void, value_type: u32, value_ptr: *mut c_void) -> bool;
}

// =============================================================================
// Objective-C Runtime FFI for NSWorkspace/NSRunningApplication
// =============================================================================

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

#[link(name = "objc", kind = "dylib")]
unsafe extern "C" {
    fn objc_getClass(name: *const i8) -> *mut c_void;
    fn sel_registerName(name: *const i8) -> *mut c_void;
    fn objc_msgSend(obj: *mut c_void, sel: *mut c_void, ...) -> *mut c_void;
}

// Helper macros for Objective-C calls
macro_rules! class {
    ($name:expr) => {
        unsafe { objc_getClass(concat!($name, "\0").as_ptr() as *const i8) }
    };
}

macro_rules! sel {
    ($name:expr) => {
        unsafe { sel_registerName(concat!($name, "\0").as_ptr() as *const i8) }
    };
}

macro_rules! msg_send {
    ($obj:expr, $sel:expr) => {
        unsafe { objc_msgSend($obj, sel!($sel)) }
    };
    ($obj:expr, $sel:expr, $arg1:expr) => {
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void =
                std::mem::transmute(objc_msgSend as *const ());
            f($obj, sel!($sel), $arg1 as *mut c_void)
        }
    };
}

// =============================================================================
// Application Management (using NSRunningApplication)
// =============================================================================

/// Known bundle IDs for test applications.
/// Using bundle IDs is more reliable than localized names.
fn get_bundle_id_for_app(app_name: &str) -> Option<&'static str> {
    match app_name {
        "Dictionary" => Some("com.apple.Dictionary"),
        "TextEdit" => Some("com.apple.TextEdit"),
        "Finder" => Some("com.apple.finder"),
        "Safari" => Some("com.apple.Safari"),
        "Calculator" => Some("com.apple.calculator"),
        "Notes" => Some("com.apple.Notes"),
        "Preview" => Some("com.apple.Preview"),
        _ => None,
    }
}

/// Gets all running applications with the given bundle ID.
fn get_running_apps_by_bundle_id(bundle_id: &str) -> Vec<*mut c_void> {
    unsafe {
        let workspace = msg_send!(class!("NSWorkspace"), "sharedWorkspace");
        let running_apps = msg_send!(workspace, "runningApplications");

        let count: isize = std::mem::transmute(msg_send!(running_apps, "count"));
        let mut matching = Vec::new();

        for i in 0..count {
            let app: *mut c_void = {
                let f: unsafe extern "C" fn(*mut c_void, *mut c_void, isize) -> *mut c_void =
                    std::mem::transmute(objc_msgSend as *const ());
                f(running_apps, sel!("objectAtIndex:"), i)
            };

            let app_bundle_id = msg_send!(app, "bundleIdentifier");
            if !app_bundle_id.is_null() {
                let utf8: *const i8 = std::mem::transmute(msg_send!(app_bundle_id, "UTF8String"));
                if !utf8.is_null() {
                    let bid = std::ffi::CStr::from_ptr(utf8).to_string_lossy();
                    if bid == bundle_id {
                        matching.push(app);
                    }
                }
            }
        }

        matching
    }
}
/// Gets all running applications with the given name.
/// Falls back to bundle ID matching for known apps.
fn get_running_apps_by_name(app_name: &str) -> Vec<*mut c_void> {
    // First try bundle ID (more reliable)
    if let Some(bundle_id) = get_bundle_id_for_app(app_name) {
        let apps = get_running_apps_by_bundle_id(bundle_id);
        if !apps.is_empty() {
            return apps;
        }
    }

    // Fall back to localized name matching
    unsafe {
        let workspace = msg_send!(class!("NSWorkspace"), "sharedWorkspace");
        let running_apps = msg_send!(workspace, "runningApplications");

        let count: isize = std::mem::transmute(msg_send!(running_apps, "count"));
        let mut matching = Vec::new();

        for i in 0..count {
            let app: *mut c_void = {
                let f: unsafe extern "C" fn(*mut c_void, *mut c_void, isize) -> *mut c_void =
                    std::mem::transmute(objc_msgSend as *const ());
                f(running_apps, sel!("objectAtIndex:"), i)
            };

            let localized_name = msg_send!(app, "localizedName");
            if !localized_name.is_null() {
                let utf8: *const i8 = std::mem::transmute(msg_send!(localized_name, "UTF8String"));
                if !utf8.is_null() {
                    let name = std::ffi::CStr::from_ptr(utf8).to_string_lossy();
                    if name == app_name {
                        matching.push(app);
                    }
                }
            }
        }

        matching
    }
}

/// Checks if an application is currently running by name.
///
/// First tries `pgrep` (more reliable in test contexts), then falls back to NSWorkspace.
pub fn is_app_running(app_name: &str) -> bool {
    // Try pgrep first (more reliable)
    if let Ok(output) = std::process::Command::new("pgrep").arg("-x").arg(app_name).output() {
        if output.status.success() {
            return true;
        }
    }

    // Fall back to NSWorkspace
    !get_running_apps_by_name(app_name).is_empty()
}

/// Gets the process ID of an application by name (or bundle ID for known apps).
///
/// First tries `pgrep` (more reliable in test contexts), then falls back to NSWorkspace.
pub fn get_app_pid(app_name: &str) -> Option<i32> {
    // Try pgrep first (more reliable)
    if let Ok(output) = std::process::Command::new("pgrep").arg("-x").arg(app_name).output() {
        if output.status.success() {
            let pid_str = String::from_utf8_lossy(&output.stdout);
            // pgrep returns one PID per line, take the first one
            if let Some(first_line) = pid_str.lines().next() {
                if let Ok(pid) = first_line.trim().parse::<i32>() {
                    return Some(pid);
                }
            }
        }
    }

    // Fall back to NSWorkspace
    let apps = get_running_apps_by_name(app_name);
    apps.first().map(|app| unsafe {
        let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32 =
            std::mem::transmute(objc_msgSend as *const ());
        f(*app, sel!("processIdentifier"))
    })
}

/// Launches an application by name using NSWorkspace.
pub fn launch_app(app_name: &str) -> bool {
    unsafe {
        let workspace = msg_send!(class!("NSWorkspace"), "sharedWorkspace");

        // Create NSString for app name
        let ns_string_class = class!("NSString");
        let app_name_cstr = std::ffi::CString::new(app_name).unwrap();
        let ns_app_name: *mut c_void = {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *const i8) -> *mut c_void =
                std::mem::transmute(objc_msgSend as *const ());
            f(
                ns_string_class,
                sel!("stringWithUTF8String:"),
                app_name_cstr.as_ptr(),
            )
        };

        // launchApplication: returns BOOL
        let result: i8 = {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i8 =
                std::mem::transmute(objc_msgSend as *const ());
            f(workspace, sel!("launchApplication:"), ns_app_name)
        };

        result != 0
    }
}

/// Activates (brings to front) an application by name.
pub fn activate_app(app_name: &str) -> bool {
    let apps = get_running_apps_by_name(app_name);
    if let Some(app) = apps.first() {
        unsafe {
            // activateWithOptions: NSApplicationActivateIgnoringOtherApps = 2
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void, isize) -> i8 =
                std::mem::transmute(objc_msgSend as *const ());
            let result = f(*app, sel!("activateWithOptions:"), 2);
            return result != 0;
        }
    }
    false
}

/// Terminates an application gracefully by name.
pub fn quit_app(app_name: &str) -> bool {
    let apps = get_running_apps_by_name(app_name);
    if let Some(app) = apps.first() {
        unsafe {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i8 =
                std::mem::transmute(objc_msgSend as *const ());
            let result = f(*app, sel!("terminate"));
            return result != 0;
        }
    }
    false
}

/// Force terminates an application by name.
pub fn force_kill_app(app_name: &str) {
    let apps = get_running_apps_by_name(app_name);
    for app in apps {
        unsafe {
            let _: *mut c_void = msg_send!(app, "forceTerminate");
        }
    }
}

// =============================================================================
// Window Management (using Accessibility API)
// =============================================================================

/// Gets all windows for an application using Accessibility API.
pub fn get_app_windows(app_name: &str) -> Vec<AXUIElementRef> {
    let pid = match get_app_pid(app_name) {
        Some(p) => p,
        None => return vec![],
    };

    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return vec![];
        }

        let attr = CFString::new("AXWindows");
        let mut value: *mut c_void = ptr::null_mut();
        let err =
            AXUIElementCopyAttributeValue(app_element, attr.as_concrete_TypeRef(), &mut value);

        CFRelease(app_element as *const c_void);

        if err != K_AX_ERROR_SUCCESS || value.is_null() {
            return vec![];
        }

        // Value is a CFArray of AXUIElementRef
        let array = value as CFArrayRef;
        let count = CFArrayGetCount(array);
        let mut windows = Vec::with_capacity(count as usize);

        for i in 0..count {
            let window = CFArrayGetValueAtIndex(array, i);
            if !window.is_null() {
                // Retain the window element since we're keeping a reference
                CFRetain(window);
                windows.push(window as AXUIElementRef);
            }
        }

        CFRelease(value);
        windows
    }
}

/// Gets the frame of a window using Accessibility API.
pub fn get_window_frame(window: AXUIElementRef) -> Option<Frame> {
    if window.is_null() {
        return None;
    }

    unsafe {
        // Get position
        let pos_attr = CFString::new("AXPosition");
        let mut pos_value: *mut c_void = ptr::null_mut();
        let pos_err =
            AXUIElementCopyAttributeValue(window, pos_attr.as_concrete_TypeRef(), &mut pos_value);

        if pos_err != K_AX_ERROR_SUCCESS || pos_value.is_null() {
            return None;
        }

        let mut point = core_graphics::geometry::CGPoint::new(0.0, 0.0);
        let got_point = AXValueGetValue(
            pos_value,
            K_AX_VALUE_TYPE_CG_POINT,
            &mut point as *mut _ as *mut c_void,
        );
        CFRelease(pos_value);

        if !got_point {
            return None;
        }

        // Get size
        let size_attr = CFString::new("AXSize");
        let mut size_value: *mut c_void = ptr::null_mut();
        let size_err =
            AXUIElementCopyAttributeValue(window, size_attr.as_concrete_TypeRef(), &mut size_value);

        if size_err != K_AX_ERROR_SUCCESS || size_value.is_null() {
            return None;
        }

        let mut size = core_graphics::geometry::CGSize::new(0.0, 0.0);
        let got_size = AXValueGetValue(
            size_value,
            K_AX_VALUE_TYPE_CG_SIZE,
            &mut size as *mut _ as *mut c_void,
        );
        CFRelease(size_value);

        if !got_size {
            return None;
        }

        Some(Frame {
            x: point.x as i32,
            y: point.y as i32,
            width: size.width as i32,
            height: size.height as i32,
        })
    }
}

/// Focuses a window using Accessibility API.
pub fn focus_window(window: AXUIElementRef) -> bool {
    if window.is_null() {
        return false;
    }

    unsafe {
        // Raise the window
        let action = CFString::new("AXRaise");
        let err = AXUIElementPerformAction(window, action.as_concrete_TypeRef());

        if err != K_AX_ERROR_SUCCESS {
            return false;
        }

        // Also set the main attribute
        let attr = CFString::new("AXMain");
        let true_val = CFBoolean::true_value();
        let _ = AXUIElementSetAttributeValue(
            window,
            attr.as_concrete_TypeRef(),
            true_val.as_CFTypeRef() as *const c_void,
        );

        true
    }
}

/// Closes a window using Accessibility API (via close button).
pub fn close_window(window: AXUIElementRef) -> bool {
    if window.is_null() {
        return false;
    }

    unsafe {
        // Get close button
        let attr = CFString::new("AXCloseButton");
        let mut button: *mut c_void = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(window, attr.as_concrete_TypeRef(), &mut button);

        if err != K_AX_ERROR_SUCCESS || button.is_null() {
            return false;
        }

        // Press the close button
        let action = CFString::new("AXPress");
        let result =
            AXUIElementPerformAction(button as AXUIElementRef, action.as_concrete_TypeRef());
        CFRelease(button);

        result == K_AX_ERROR_SUCCESS
    }
}

/// Gets the title of a window using Accessibility API.
pub fn get_window_title(window: AXUIElementRef) -> Option<String> {
    if window.is_null() {
        return None;
    }

    unsafe {
        let attr = CFString::new("AXTitle");
        let mut value: *mut c_void = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(window, attr.as_concrete_TypeRef(), &mut value);

        if err != K_AX_ERROR_SUCCESS || value.is_null() {
            return None;
        }

        let cf_string =
            CFString::wrap_under_create_rule(value as core_foundation::string::CFStringRef);
        Some(cf_string.to_string())
    }
}

/// Releases an AXUIElementRef.
pub fn release_window(window: AXUIElementRef) {
    if !window.is_null() {
        unsafe {
            CFRelease(window as *const c_void);
        }
    }
}

// =============================================================================
// Screen Information (using Core Graphics)
// =============================================================================

/// Information about a display screen.
#[derive(Debug, Clone)]
pub struct ScreenInfo {
    pub frame: Frame,
    pub is_main: bool,
    pub display_id: u32,
}

/// Gets the main screen frame using Core Graphics.
pub fn get_main_screen_frame() -> Frame {
    let main_display = CGDisplay::main();
    let bounds = main_display.bounds();

    Frame {
        x: bounds.origin.x as i32,
        y: bounds.origin.y as i32,
        width: bounds.size.width as i32,
        height: bounds.size.height as i32,
    }
}

// FFI for CGGetOnlineDisplayList
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGGetOnlineDisplayList(
        max_displays: u32,
        online_displays: *mut u32,
        display_count: *mut u32,
    ) -> i32;
}

/// Gets all active screens.
pub fn get_all_screens() -> Vec<ScreenInfo> {
    use core_graphics::display::CGDisplay;

    let main_id = CGDisplay::main().id;

    // Get all online displays
    let max_displays = 16u32;
    let mut display_ids = vec![0u32; max_displays as usize];
    let mut display_count = 0u32;

    unsafe {
        CGGetOnlineDisplayList(max_displays, display_ids.as_mut_ptr(), &mut display_count);
    }

    display_ids
        .into_iter()
        .take(display_count as usize)
        .map(|id| {
            let display = CGDisplay::new(id);
            let bounds = display.bounds();
            ScreenInfo {
                frame: Frame {
                    x: bounds.origin.x as i32,
                    y: bounds.origin.y as i32,
                    width: bounds.size.width as i32,
                    height: bounds.size.height as i32,
                },
                is_main: id == main_id,
                display_id: id,
            }
        })
        .collect()
}

/// Finds which screen contains the given point.
pub fn screen_containing_point(x: i32, y: i32) -> Option<ScreenInfo> {
    let screens = get_all_screens();

    for screen in screens {
        if x >= screen.frame.x
            && x < screen.frame.x + screen.frame.width
            && y >= screen.frame.y
            && y < screen.frame.y + screen.frame.height
        {
            return Some(screen);
        }
    }

    None
}

/// Finds which screen contains the center of the given frame.
pub fn screen_containing_frame(frame: &Frame) -> Option<ScreenInfo> {
    let center_x = frame.x + frame.width / 2;
    let center_y = frame.y + frame.height / 2;
    screen_containing_point(center_x, center_y)
}

// =============================================================================
// Window Creation (using Accessibility API menu interaction)
// =============================================================================

/// Helper to find a menu item by path and click it using AX API.
fn click_menu_item(app_name: &str, menu_path: &[&str]) -> bool {
    let pid = match get_app_pid(app_name) {
        Some(p) => p,
        None => return false,
    };

    unsafe {
        let app_element = AXUIElementCreateApplication(pid);
        if app_element.is_null() {
            return false;
        }

        // Get menu bar
        let menu_bar_attr = CFString::new("AXMenuBar");
        let mut menu_bar: *mut c_void = ptr::null_mut();
        let err = AXUIElementCopyAttributeValue(
            app_element,
            menu_bar_attr.as_concrete_TypeRef(),
            &mut menu_bar,
        );

        CFRelease(app_element as *const c_void);

        if err != K_AX_ERROR_SUCCESS || menu_bar.is_null() {
            return false;
        }

        let mut current_element = menu_bar;
        let mut found = true;

        for (depth, &item_name) in menu_path.iter().enumerate() {
            let children_attr = CFString::new("AXChildren");
            let mut children: *mut c_void = ptr::null_mut();
            let err = AXUIElementCopyAttributeValue(
                current_element,
                children_attr.as_concrete_TypeRef(),
                &mut children,
            );

            if depth > 0 {
                CFRelease(current_element as *const c_void);
            }

            if err != K_AX_ERROR_SUCCESS || children.is_null() {
                found = false;
                break;
            }

            let array = children as CFArrayRef;
            let count = CFArrayGetCount(array);
            let mut found_item: AXUIElementRef = ptr::null_mut();

            for i in 0..count {
                let child = CFArrayGetValueAtIndex(array, i) as AXUIElementRef;
                if child.is_null() {
                    continue;
                }

                // Get title
                let title_attr = CFString::new("AXTitle");
                let mut title: *mut c_void = ptr::null_mut();
                let t_err = AXUIElementCopyAttributeValue(
                    child,
                    title_attr.as_concrete_TypeRef(),
                    &mut title,
                );

                if t_err == K_AX_ERROR_SUCCESS && !title.is_null() {
                    let cf_title = CFString::wrap_under_create_rule(
                        title as core_foundation::string::CFStringRef,
                    );
                    if cf_title.to_string() == item_name {
                        CFRetain(child as *const c_void);
                        found_item = child;
                        break;
                    }
                }
            }

            CFRelease(children);

            if found_item.is_null() {
                found = false;
                break;
            }

            // If not the last item, we need to get the menu under this item
            if depth < menu_path.len() - 1 {
                // First press to open the menu
                let press_action = CFString::new("AXPress");
                let _ = AXUIElementPerformAction(found_item, press_action.as_concrete_TypeRef());
                thread::sleep(Duration::from_millis(100));

                // Get the submenu
                let children_attr = CFString::new("AXChildren");
                let mut submenu: *mut c_void = ptr::null_mut();
                let err = AXUIElementCopyAttributeValue(
                    found_item,
                    children_attr.as_concrete_TypeRef(),
                    &mut submenu,
                );

                if err == K_AX_ERROR_SUCCESS && !submenu.is_null() {
                    let sub_array = submenu as CFArrayRef;
                    if CFArrayGetCount(sub_array) > 0 {
                        let first_child = CFArrayGetValueAtIndex(sub_array, 0) as AXUIElementRef;
                        CFRetain(first_child as *const c_void);
                        CFRelease(submenu);
                        CFRelease(found_item as *const c_void);
                        current_element = first_child;
                        continue;
                    }
                    CFRelease(submenu);
                }

                CFRelease(found_item as *const c_void);
                found = false;
                break;
            } else {
                // Last item - click it
                let press_action = CFString::new("AXPress");
                let result =
                    AXUIElementPerformAction(found_item, press_action.as_concrete_TypeRef());
                CFRelease(found_item as *const c_void);
                found = result == K_AX_ERROR_SUCCESS;
            }
        }

        if !menu_bar.is_null() && current_element != menu_bar {
            // menu_bar already released in the loop
        }

        found
    }
}

/// Creates a new window for Dictionary app.
///
/// Dictionary creates a window automatically when launched, so we just need
/// to ensure it's running. If already running with windows, we create a new one.
pub fn create_dictionary_window() -> bool {
    let was_running = is_app_running("Dictionary");
    let initial_window_count = if was_running {
        get_app_windows("Dictionary").len()
    } else {
        0
    };

    // Launch if not running
    if !was_running {
        if !launch_app("Dictionary") {
            return false;
        }
        // Poll until Dictionary is running
        if !poll_until(|| is_app_running("Dictionary"), Duration::from_secs(5)) {
            return false;
        }
        // Dictionary auto-creates a window on launch, poll until it appears
        if !poll_until(
            || !get_app_windows("Dictionary").is_empty(),
            Duration::from_secs(3),
        ) {
            return false;
        }
        // Window was auto-created by launch
        return true;
    }

    // Dictionary is already running - create a new window via menu
    activate_app("Dictionary");

    // Poll until menu is ready (Dictionary has a window or short timeout)
    poll_until(
        || !get_app_windows("Dictionary").is_empty(),
        Duration::from_secs(1),
    );

    // Create window via File > New Window menu
    if click_menu_item("Dictionary", &["File", "New Window"]) {
        // Poll until a new window appears
        return poll_until(
            || get_app_windows("Dictionary").len() > initial_window_count,
            Duration::from_secs(3),
        );
    }

    false
}

/// Polls until a condition is true or timeout.
fn poll_until<F: Fn() -> bool>(condition: F, timeout: Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        if condition() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Creates a new window for TextEdit app.
///
/// Assumes TextEdit has been prepared via App::prepare() which clears
/// session state. Simply launches (if needed), activates, and creates window.
pub fn create_textedit_window() -> bool {
    // Launch if not running, poll until running
    if !is_app_running("TextEdit") {
        if !launch_app("TextEdit") {
            return false;
        }
        if !poll_until(|| is_app_running("TextEdit"), Duration::from_secs(5)) {
            return false;
        }
    }

    // Activate TextEdit
    activate_app("TextEdit");

    // Poll until TextEdit is ready
    if !poll_until(|| get_app_pid("TextEdit").is_some(), Duration::from_secs(2)) {
        return false;
    }

    // Create window via File > New menu (TextEdit uses "New" not "New Window")
    click_menu_item("TextEdit", &["File", "New"])
}

/// Sends a keyboard shortcut using CGEvent (system-wide).
///
/// Note: This posts events system-wide, not to a specific app.
/// The target app should be frontmost when calling this.
fn send_keyboard_shortcut(key: &str, cmd: bool, shift: bool, option: bool) -> bool {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    // Map key to keycode
    let keycode: CGKeyCode = match key.to_lowercase().as_str() {
        "n" => 0x2D, // kVK_ANSI_N
        "w" => 0x0D, // kVK_ANSI_W
        "q" => 0x0C, // kVK_ANSI_Q
        _ => return false,
    };

    let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Create key down event
    let key_down = match CGEvent::new_keyboard_event(source.clone(), keycode, true) {
        Ok(e) => e,
        Err(_) => return false,
    };

    // Create key up event
    let key_up = match CGEvent::new_keyboard_event(source, keycode, false) {
        Ok(e) => e,
        Err(_) => return false,
    };

    // Set modifier flags
    let mut flags = CGEventFlags::empty();
    if cmd {
        flags |= CGEventFlags::CGEventFlagCommand;
    }
    if shift {
        flags |= CGEventFlags::CGEventFlagShift;
    }
    if option {
        flags |= CGEventFlags::CGEventFlagAlternate;
    }

    key_down.set_flags(flags);
    key_up.set_flags(flags);

    // Post events to the HID system (frontmost app receives them)
    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));
    key_up.post(CGEventTapLocation::HID);

    true
}
