//! Window information and querying.
//!
//! This module provides functions to discover and query windows.

use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::window::{
    CGWindowListCopyWindowInfo, kCGNullWindowID, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly,
};

use crate::tiling::accessibility::is_accessibility_enabled;
use crate::tiling::error::TilingError;
use crate::tiling::state::{ManagedWindow, WindowFrame};

/// Result type for window operations.
pub type WindowResult<T> = Result<T, TilingError>;

/// Keys for window info dictionary.
mod keys {
    pub const WINDOW_NUMBER: &str = "kCGWindowNumber";
    pub const WINDOW_OWNER_PID: &str = "kCGWindowOwnerPID";
    pub const WINDOW_OWNER_NAME: &str = "kCGWindowOwnerName";
    pub const WINDOW_NAME: &str = "kCGWindowName";
    pub const WINDOW_BOUNDS: &str = "kCGWindowBounds";
    pub const WINDOW_LAYER: &str = "kCGWindowLayer";
}

/// Applications to exclude from window management.
const EXCLUDED_APPS: &[&str] = &[
    "Dock",           // Dock
    "Borders",        // Window borders utility (capitalized)
    "borders",        // Window borders utility (lowercase, from process name)
    "SystemUIServer", // Menu bar extras
    "Control Center", // Control center
    "Notification Center",
    "Spotlight",
    "Barba",     // Our own app (display name)
    "barba-app", // Our own app (process name)
    // Electron/WebView helper processes
    "Microsoft Teams WebView",
    "Slack Helper",
    "Discord Helper",
    "Electron Helper",
    "Chromium Helper",
    "Google Chrome Helper",
];

/// Gets all visible windows on screen.
pub fn get_all_windows() -> WindowResult<Vec<ManagedWindow>> { get_windows_with_options(true) }

/// Gets the PID of the frontmost (focused) application.
/// This captures just the PID without needing to query window details,
/// which is useful to capture focus state before operations that might change it.
pub fn get_frontmost_app_pid() -> Option<i32> {
    use objc2_app_kit::NSWorkspace;

    use crate::tiling::accessibility::AccessibilityElement;

    // Try using system-wide element to get the focused application
    let system_element = AccessibilityElement::system_wide();
    if let Ok(app_element) = system_element
        .get_element_attribute(crate::tiling::accessibility::attributes::FOCUSED_APPLICATION)
        && let Ok(pid) = app_element.pid()
        && pid > 0
    {
        return Some(pid);
    }

    // Fallback to NSWorkspace
    let workspace = NSWorkspace::sharedWorkspace();
    workspace.frontmostApplication().map(|app| app.processIdentifier())
}

/// Gets all windows including hidden/off-screen ones.
/// This is useful after unhiding apps when their windows may not yet be "on screen".
pub fn get_all_windows_including_hidden() -> WindowResult<Vec<ManagedWindow>> {
    get_windows_with_options(false)
}

/// Gets windows with configurable on-screen filtering.
fn get_windows_with_options(on_screen_only: bool) -> WindowResult<Vec<ManagedWindow>> {
    use core_graphics::window::kCGWindowListOptionAll;

    let options = if on_screen_only {
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements
    } else {
        // When getting all windows, don't exclude desktop elements either
        kCGWindowListOptionAll
    };

    let window_list = unsafe { CGWindowListCopyWindowInfo(options, kCGNullWindowID) };

    if window_list.is_null() {
        return Err(TilingError::OperationFailed(
            "Failed to get window list".to_string(),
        ));
    }

    // SAFETY: We just checked that window_list is not null
    let windows_array: CFArray<CFDictionary<CFString, CFType>> =
        unsafe { CFArray::wrap_under_create_rule(window_list) };

    let mut managed_windows = Vec::new();

    for i in 0..windows_array.len() {
        if let Some(window_dict) = windows_array.get(i)
            && let Some(window) = parse_window_dict(&window_dict)
        {
            // Filter out excluded apps and windows on layer != 0 (normal windows)
            if !EXCLUDED_APPS.iter().any(|&app| window.app_name == app) {
                managed_windows.push(window);
            }
        }
    }

    Ok(managed_windows)
}

/// Gets a specific window by ID.
pub fn get_window_by_id(window_id: u64) -> WindowResult<ManagedWindow> {
    get_all_windows()?
        .into_iter()
        .find(|w| w.id == window_id)
        .ok_or(TilingError::WindowNotFound(window_id))
}

/// Gets the currently focused window using the Accessibility API.
/// This uses the system-wide focused application and `AXFocusedWindow`
/// to get the actual focused window.
#[allow(clippy::items_after_statements)]
pub fn get_focused_window() -> WindowResult<ManagedWindow> {
    if !is_accessibility_enabled() {
        return Err(TilingError::AccessibilityNotAuthorized);
    }

    use objc2_app_kit::NSWorkspace;

    use crate::tiling::accessibility::AccessibilityElement;

    // Try using system-wide element to get the focused application
    // This is more reliable for multi-monitor setups
    let frontmost_pid: i32 = {
        let system_element = AccessibilityElement::system_wide();
        system_element
            .get_element_attribute(crate::tiling::accessibility::attributes::FOCUSED_APPLICATION)
            .map_or(-1, |app_element| app_element.pid().unwrap_or(-1))
    };

    // Fallback to NSWorkspace if system-wide approach failed
    let frontmost_pid = if frontmost_pid > 0 {
        frontmost_pid
    } else {
        let workspace = NSWorkspace::sharedWorkspace();
        let Some(frontmost_app) = workspace.frontmostApplication() else {
            return Err(TilingError::WindowNotFound(0));
        };
        frontmost_app.processIdentifier()
    };

    // Use Accessibility API to get the actual focused window of this app
    let app_element = AccessibilityElement::application(frontmost_pid);
    let focused_ax_window = app_element.get_focused_window()?;
    let focused_frame = focused_ax_window.get_frame()?;

    // Get all windows and find the one matching the focused window's position/size
    let windows = get_all_windows()?;

    // Filter to windows of the frontmost app first
    let app_windows: Vec<_> = windows.into_iter().filter(|w| w.pid == frontmost_pid).collect();

    // If there's only one window, return it
    if app_windows.len() == 1 {
        return Ok(app_windows.into_iter().next().unwrap());
    }

    // Find window matching the focused window's frame
    for window in app_windows {
        // Match by position with a small tolerance (sometimes AX and CG report slightly different values)
        if (window.frame.x - focused_frame.x).abs() <= 5
            && (window.frame.y - focused_frame.y).abs() <= 5
            && window.frame.width.abs_diff(focused_frame.width) <= 5
            && window.frame.height.abs_diff(focused_frame.height) <= 5
        {
            return Ok(window);
        }
    }

    // Fallback: try to get any window from the frontmost app
    get_all_windows()?
        .into_iter()
        .find(|w| w.pid == frontmost_pid)
        .ok_or(TilingError::WindowNotFound(0))
}

/// Parses a window info dictionary into a `ManagedWindow`.
fn parse_window_dict(dict: &CFDictionary<CFString, CFType>) -> Option<ManagedWindow> {
    // Get window ID
    let window_id = get_number_value(dict, keys::WINDOW_NUMBER)?;

    // Get PID
    #[allow(clippy::cast_possible_truncation)]
    let pid = get_number_value(dict, keys::WINDOW_OWNER_PID)? as i32;

    // Get layer - include normal windows (layer 0) and floating panels (layer 3)
    // Layer 3 is used by some apps for their main windows, but NOT Finder
    // (Finder's layer 3 windows are floating panels like the sidebar)
    let layer = get_number_value(dict, keys::WINDOW_LAYER).unwrap_or(0);
    if layer != 0 && layer != 3 {
        return None;
    }

    // Get app name
    let app_name = get_string_value(dict, keys::WINDOW_OWNER_NAME).unwrap_or_default();

    // Finder layer 3 windows are floating panels (sidebar, etc.), not folder windows
    if app_name == "Finder" && layer == 3 {
        return None;
    }

    // Get window title
    let title = get_string_value(dict, keys::WINDOW_NAME).unwrap_or_default();

    // Get bounds
    let frame = get_bounds(dict).unwrap_or_default();

    // Skip Finder's Desktop windows (the desktop icon overlay).
    // These windows have an empty title and a negative Y position (positioned above/behind the screen).
    // Real Finder folder windows are positioned in the visible work area (y >= 0).
    if app_name == "Finder" && title.is_empty() && frame.y < 0 {
        return None;
    }

    // Skip windows with no size (probably invisible)
    if frame.width == 0 || frame.height == 0 {
        return None;
    }

    // Skip windows that are too small to be real application windows
    // These are typically toolbars, sidebars, popups, or other UI elements
    // Minimum size: 100x100 pixels
    if frame.width < 100 || frame.height < 100 {
        return None;
    }

    // Get bundle ID from PID
    let bundle_id = get_bundle_id_for_pid(pid);

    Some(ManagedWindow {
        id: window_id,
        title,
        app_name,
        bundle_id,
        class: None,
        pid,
        workspace: String::new(), // Will be assigned by workspace manager
        is_floating: false,
        is_minimized: false, // CG doesn't report this, need AX
        is_fullscreen: false,
        is_hidden: false,
        frame,
    })
}

/// Checks if a window appears to be a Picture-in-Picture (`PiP`) window.
///
/// `PiP` windows are typically:
/// - Small floating video overlay windows
/// - Have titles containing "Picture in Picture", "PIP", or are empty/generic
/// - Common from Safari, Chrome, Firefox, and media apps
///
/// These should generally be excluded from tiling layouts but allowed in floating mode.
#[must_use]
pub fn is_pip_window(window: &ManagedWindow) -> bool {
    // Check for PiP-related titles
    let title_lower = window.title.to_lowercase();
    if title_lower.contains("picture in picture")
        || title_lower.contains("picture-in-picture")
        || title_lower == "pip"
    {
        return true;
    }

    // Common browsers and apps that support PiP
    let pip_capable_apps = [
        "com.apple.Safari",
        "com.google.Chrome",
        "org.mozilla.firefox",
        "com.microsoft.edgemac",
        "com.microsoft.edgemac.Dev",
        "com.apple.TV",
        "com.apple.Music",
        "com.spotify.client",
        "tv.twitch.studio",
        "com.netflix.Netflix",
        "com.apple.QuickTimePlayerX",
    ];

    if let Some(ref bundle_id) = window.bundle_id {
        // For PiP-capable apps, check if this is a small window with empty/generic title
        // (PiP windows often have empty titles or just show video title)
        if pip_capable_apps.iter().any(|&app| bundle_id.contains(app)) {
            // Small window from a PiP-capable app with empty title is likely PiP
            if window.title.is_empty() {
                return true;
            }

            // Safari's PiP windows are layer 3
            if bundle_id.contains("Safari") {
                return true;
            }
        }
    }

    false
}

/// Checks if a window is a dialog, sheet, or other non-tileable window type.
///
/// Uses the macOS Accessibility API to check the window's subrole.
/// Returns `true` for dialogs, sheets, system dialogs, and floating windows
/// (palettes, inspectors, etc.) that should not be included in tiling layouts.
///
/// This is a relatively expensive check as it requires accessibility API calls,
/// so it should be used selectively (e.g., when a new window appears).
#[must_use]
pub fn is_dialog_or_sheet(window: &ManagedWindow) -> bool {
    use crate::tiling::accessibility::AccessibilityElement;

    if !is_accessibility_enabled() {
        return false;
    }

    let app = AccessibilityElement::application(window.pid);
    let Ok(ax_windows) = app.get_windows() else {
        return false;
    };

    // Find the matching accessibility window by position/size
    for ax_window in ax_windows {
        if let Ok(frame) = ax_window.get_frame() {
            // Match by position (within a small tolerance)
            if (frame.x - window.frame.x).abs() <= 2
                && (frame.y - window.frame.y).abs() <= 2
                && frame.width.abs_diff(window.frame.width) <= 2
                && frame.height.abs_diff(window.frame.height) <= 2
            {
                return ax_window.is_dialog_or_sheet();
            }
        }
    }

    false
}

/// Gets a number value from a dictionary.
fn get_number_value(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<u64> {
    let key_cf = CFString::new(key);

    dict.find(&key_cf).and_then(|value| {
        // Try to cast to CFNumber
        #[allow(clippy::cast_sign_loss)]
        value.downcast::<CFNumber>().and_then(|num| num.to_i64().map(|n| n as u64))
    })
}

/// Gets a string value from a dictionary.
fn get_string_value(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<String> {
    let key_cf = CFString::new(key);

    dict.find(&key_cf)
        .and_then(|value| value.downcast::<CFString>().map(|s| s.to_string()))
}

/// Gets bounds from a window dictionary using Core Foundation.
fn get_bounds(dict: &CFDictionary<CFString, CFType>) -> Option<WindowFrame> {
    use std::ffi::c_void;

    use core_foundation::dictionary::CFDictionaryRef;

    // Link to Core Graphics function
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGRectMakeWithDictionaryRepresentation(
            dict: *const c_void,
            rect: *mut core_graphics::geometry::CGRect,
        ) -> bool;
    }

    let bounds_key = CFString::new(keys::WINDOW_BOUNDS);

    dict.find(&bounds_key).and_then(|bounds_value| {
        let dict_ref = bounds_value.as_concrete_TypeRef() as CFDictionaryRef;
        let mut rect = core_graphics::geometry::CGRect::default();

        let success =
            unsafe { CGRectMakeWithDictionaryRepresentation(dict_ref.cast(), &raw mut rect) };

        if success {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            Some(WindowFrame::new(
                rect.origin.x as i32,
                rect.origin.y as i32,
                rect.size.width as u32,
                rect.size.height as u32,
            ))
        } else {
            None
        }
    })
}

/// Gets the bundle identifier for a process.
pub fn get_bundle_id_for_pid(pid: i32) -> Option<String> {
    // Use NSRunningApplication to get bundle ID
    // This requires linking with AppKit, which Tauri already does
    use objc2_app_kit::NSRunningApplication;

    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    let bundle_id = app.bundleIdentifier()?;
    Some(bundle_id.to_string())
}

/// Represents a running application.
pub struct RunningApp {
    pub pid: i32,
    pub bundle_id: Option<String>,
    pub name: String,
}

/// Gets all running applications (including hidden ones).
/// This uses NSWorkspace.runningApplications which includes hidden apps.
pub fn get_all_running_apps() -> Vec<RunningApp> {
    use objc2_app_kit::NSWorkspace;

    let mut apps = Vec::new();

    let workspace = NSWorkspace::sharedWorkspace();
    let running_apps = workspace.runningApplications();

    for app in running_apps {
        // Get PID
        let pid = app.processIdentifier();

        // Get bundle ID
        let bundle_id = app.bundleIdentifier().map(|s| s.to_string());

        // Get localized name
        let name = app.localizedName().map_or(String::new(), |s| s.to_string());

        // Skip excluded apps
        if EXCLUDED_APPS.iter().any(|&excluded| name == excluded) {
            continue;
        }

        apps.push(RunningApp { pid, bundle_id, name });
    }

    apps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tiling::state::WindowFrame;

    fn create_test_window(
        title: &str,
        bundle_id: Option<&str>,
        width: u32,
        height: u32,
    ) -> ManagedWindow {
        ManagedWindow {
            id: 1,
            title: title.to_string(),
            app_name: "Test App".to_string(),
            bundle_id: bundle_id.map(String::from),
            class: None,
            pid: 123,
            workspace: "main".to_string(),
            is_floating: false,
            is_minimized: false,
            is_fullscreen: false,
            is_hidden: false,
            frame: WindowFrame::new(0, 0, width, height),
        }
    }

    #[test]
    fn test_excluded_apps_not_empty() {
        assert!(!EXCLUDED_APPS.is_empty());
        assert!(EXCLUDED_APPS.contains(&"Dock"));
    }

    #[test]
    fn test_pip_window_by_title() {
        let window = create_test_window("Picture in Picture", None, 400, 225);
        assert!(is_pip_window(&window));
    }

    #[test]
    fn test_pip_window_by_title_hyphenated() {
        let window = create_test_window("Picture-in-Picture", None, 400, 225);
        assert!(is_pip_window(&window));
    }

    #[test]
    fn test_pip_window_by_title_case_insensitive() {
        let window = create_test_window("PICTURE IN PICTURE", None, 400, 225);
        assert!(is_pip_window(&window));
    }

    #[test]
    fn test_pip_window_title_pip() {
        let window = create_test_window("PIP", None, 400, 225);
        assert!(is_pip_window(&window));
    }

    #[test]
    fn test_pip_window_safari_small_empty_title() {
        let window = create_test_window("", Some("com.apple.Safari"), 400, 225);
        assert!(is_pip_window(&window));
    }

    #[test]
    fn test_pip_window_chrome_empty_title() {
        let window = create_test_window("", Some("com.google.Chrome"), 400, 225);
        assert!(is_pip_window(&window));
    }

    #[test]
    fn test_pip_window_firefox_empty_title() {
        let window = create_test_window("", Some("org.mozilla.firefox"), 400, 225);
        assert!(is_pip_window(&window));
    }

    #[test]
    fn test_not_pip_window_normal_app() {
        // Regular app window
        let window = create_test_window("My Document.txt", Some("com.apple.TextEdit"), 600, 400);
        assert!(!is_pip_window(&window));
    }

    #[test]
    fn test_not_pip_window_non_pip_app_empty_title() {
        // Empty title but not a PiP-capable app
        let window = create_test_window("", Some("com.example.random"), 400, 225);
        assert!(!is_pip_window(&window));
    }

    #[test]
    fn test_pip_window_tv_app() {
        let window = create_test_window("", Some("com.apple.TV"), 400, 225);
        assert!(is_pip_window(&window));
    }

    #[test]
    fn test_pip_window_spotify() {
        let window = create_test_window("", Some("com.spotify.client"), 400, 225);
        assert!(is_pip_window(&window));
    }
}
