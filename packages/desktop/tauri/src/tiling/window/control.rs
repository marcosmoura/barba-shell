//! Window control operations.
//!
//! This module provides functions to manipulate window position, size, and state.

use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};

use super::info::{get_all_windows_including_hidden, get_window_by_id};
use crate::tiling::accessibility::{AccessibilityElement, is_accessibility_enabled};
use crate::tiling::error::TilingError;
use crate::tiling::state::{ManagedWindow, WindowFrame};
use crate::tiling::window::is_pip_window;

/// Result type for window control operations.
pub type ControlResult<T> = Result<T, TilingError>;

/// Gets an accessibility element for a window using its cached info.
/// This is faster than `get_ax_element_for_window` as it doesn't query window info.
fn get_ax_element_with_window_info(window: &ManagedWindow) -> ControlResult<AccessibilityElement> {
    if !is_accessibility_enabled() {
        return Err(TilingError::AccessibilityNotAuthorized);
    }

    // Create app element and find the window
    let app = AccessibilityElement::application(window.pid);

    // Get all windows and find the one matching our ID
    let ax_windows = app.get_windows()?;

    // If there's only one window, use it directly without position matching
    if ax_windows.len() == 1 {
        return Ok(ax_windows.into_iter().next().unwrap());
    }

    // We need to match by position/size since CGWindowID isn't directly accessible via AX
    let mut best_match: Option<AccessibilityElement> = None;
    for ax_window in ax_windows {
        if let Ok(frame) = ax_window.get_frame() {
            // Match by position (within a small tolerance)
            if (frame.x - window.frame.x).abs() <= 2
                && (frame.y - window.frame.y).abs() <= 2
                && frame.width.abs_diff(window.frame.width) <= 2
                && frame.height.abs_diff(window.frame.height) <= 2
            {
                return Ok(ax_window);
            }
            // Keep the first window as fallback
            if best_match.is_none() {
                best_match = Some(ax_window);
            }
        }
    }

    // If we didn't find an exact match but have windows, return the first one
    best_match.ok_or(TilingError::WindowNotFound(window.id))
}

/// Gets an accessibility element for a window.
fn get_ax_element_for_window(window_id: u64) -> ControlResult<AccessibilityElement> {
    if !is_accessibility_enabled() {
        return Err(TilingError::AccessibilityNotAuthorized);
    }

    // Get the window info to find the PID
    // Try on-screen windows first, then fall back to all windows (including hidden)
    let window = get_window_by_id(window_id).or_else(|_| {
        get_all_windows_including_hidden()?
            .into_iter()
            .find(|w| w.id == window_id)
            .ok_or(TilingError::WindowNotFound(window_id))
    })?;

    // Create app element and find the window
    let app = AccessibilityElement::application(window.pid);

    // Get all windows and find the one matching our ID
    let ax_windows = app.get_windows()?;

    // If there's only one window, use it directly without position matching
    if ax_windows.len() == 1 {
        return Ok(ax_windows.into_iter().next().unwrap());
    }

    // We need to match by position/size since CGWindowID isn't directly accessible via AX
    // First pass: try to find an exact match
    let mut best_match: Option<AccessibilityElement> = None;
    for ax_window in ax_windows {
        if let Ok(frame) = ax_window.get_frame() {
            // Match by position (within a small tolerance)
            if (frame.x - window.frame.x).abs() <= 2
                && (frame.y - window.frame.y).abs() <= 2
                && frame.width.abs_diff(window.frame.width) <= 2
                && frame.height.abs_diff(window.frame.height) <= 2
            {
                return Ok(ax_window);
            }
            // Keep the first window as fallback
            if best_match.is_none() {
                best_match = Some(ax_window);
            }
        }
    }

    // If we didn't find an exact match but have windows, return the first one
    // This handles cases where the window position/size hasn't been updated yet
    best_match.ok_or(TilingError::WindowNotFound(window_id))
}

/// Closes a window using the Accessibility API.
///
/// This performs the `AXPress` action on the window's close button.
pub fn close_window(window_id: u64) -> ControlResult<()> {
    let ax_element = get_ax_element_for_window(window_id)?;

    // Get the close button (first child of window with AXCloseButton subrole)
    // We use AXPress action on the window which triggers the close
    ax_element.perform_action("AXRaise")?;

    // Try to find and press the close button
    // The close button is typically accessed via the window's AXCloseButton attribute
    let close_button = ax_element.get_element_attribute("AXCloseButton")?;
    close_button.perform_action("AXPress")
}

/// Resizes a window.
pub fn resize_window(window_id: u64, width: u32, height: u32) -> ControlResult<()> {
    let ax_element = get_ax_element_for_window(window_id)?;
    ax_element.set_size(f64::from(width), f64::from(height))
}

/// Sets a window's frame (position and size).
pub fn set_window_frame(window_id: u64, frame: &WindowFrame) -> ControlResult<()> {
    let ax_element = get_ax_element_for_window(window_id)?;
    ax_element.set_frame(frame)
}

/// Focuses a window (brings it to front) using cached window info.
/// This is much faster than `focus_window` as it doesn't query the window list.
pub fn focus_window_fast(window: &ManagedWindow) -> ControlResult<()> {
    if is_pip_window(window) {
        return Err(TilingError::OperationFailed(
            "Cannot focus Picture-in-Picture windows".to_string(),
        ));
    }

    // First, activate the application
    activate_app(window.pid)?;

    // Then focus the specific window using cached info
    let ax_element = get_ax_element_with_window_info(window)?;
    ax_element.focus()
}

/// Focuses a window (brings it to front).
/// Note: Prefer `focus_window_fast` when you have the `ManagedWindow` available.
pub fn focus_window(window_id: u64) -> ControlResult<()> {
    let window = get_window_by_id(window_id)?;
    focus_window_fast(&window)
}

/// Activates an application by PID.
fn activate_app(pid: i32) -> ControlResult<()> {
    let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
        return Err(TilingError::OperationFailed(
            "Failed to find application".to_string(),
        ));
    };

    // Activate the application and bring all its windows forward
    // Note: ActivateIgnoringOtherApps was deprecated in macOS 14, but we still
    // need to activate the app. Use ActivateAllWindows for now.
    app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);

    Ok(())
}

/// Hides an application (all its windows) using the `AXHidden` attribute.
/// This is equivalent to pressing Cmd+H.
pub fn hide_app(pid: i32) -> ControlResult<()> {
    if !is_accessibility_enabled() {
        return Err(TilingError::AccessibilityNotAuthorized);
    }

    let app = AccessibilityElement::application(pid);
    app.set_bool_attribute(crate::tiling::accessibility::attributes::HIDDEN, true)
}

/// Unhides an application (shows all its windows) using the `AXHidden` attribute.
pub fn unhide_app(pid: i32) -> ControlResult<()> {
    if !is_accessibility_enabled() {
        return Err(TilingError::AccessibilityNotAuthorized);
    }

    let app = AccessibilityElement::application(pid);
    app.set_bool_attribute(crate::tiling::accessibility::attributes::HIDDEN, false)
}
