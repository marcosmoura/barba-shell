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

    // If there's only one window, use it directly without matching
    if ax_windows.len() == 1 {
        return Ok(ax_windows.into_iter().next().unwrap());
    }

    // Collect AX window titles for matching
    let ax_titles: Vec<Option<String>> = ax_windows
        .iter()
        .map(super::super::accessibility::AccessibilityElement::get_title)
        .collect();

    // Try to match by title first (most reliable for same-app windows)
    if !window.title.is_empty() {
        for (i, ax_window) in ax_windows.into_iter().enumerate() {
            if ax_titles[i].as_ref().is_some_and(|t| t == &window.title) {
                return Ok(ax_window);
            }
        }
        // Title matching failed, get windows again for position matching
        let ax_windows = app.get_windows()?;
        return match_window_by_position(ax_windows, window);
    }

    // Fallback: match by position/size
    match_window_by_position(ax_windows, window)
}

/// Matches a window by position/size from a list of accessibility elements.
fn match_window_by_position(
    ax_windows: Vec<AccessibilityElement>,
    window: &ManagedWindow,
) -> ControlResult<AccessibilityElement> {
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

    // Get the accessibility element for the window first
    let ax_element = get_ax_element_with_window_info(window)?;

    // Raise and focus the window BEFORE activating the app
    // This ensures the correct window is on top when the app becomes active
    ax_element.focus()?;

    // Then activate the application to bring it to the foreground
    activate_app(window.pid)?;

    // Focus the window again after activation to ensure it's the main window
    ax_element.focus()
}

/// Focuses a window (brings it to front).
/// Note: Prefer `focus_window_fast` when you have the `ManagedWindow` available.
pub fn focus_window(window_id: u64) -> ControlResult<()> {
    let window = get_window_by_id(window_id)?;
    focus_window_fast(&window)
}

/// Cycles to the next or previous window of the given app.
///
/// This uses the AX window list which is ordered by z-order (front to back).
/// For "next": focuses the second window (the one behind the current front window)
/// For "previous": focuses the last window (will become front after focus)
///
/// This is more reliable than trying to match specific windows by title,
/// since some apps (like Edge) report different titles via `CGWindowList` vs AX.
pub fn cycle_app_window(pid: i32, direction: &str) -> ControlResult<()> {
    if !is_accessibility_enabled() {
        return Err(TilingError::AccessibilityNotAuthorized);
    }

    let app = AccessibilityElement::application(pid);
    let ax_windows = app.get_windows()?;

    if ax_windows.len() < 2 {
        return Ok(()); // Nothing to cycle if only one window
    }

    // AX windows are ordered by z-order (front to back)
    // For "next": focus the second window (index 1)
    // For "previous": focus the last window
    let target_index = if direction == "next" {
        1 // The window right behind the current front window
    } else {
        ax_windows.len() - 1 // The backmost window
    };

    let target_window =
        ax_windows.into_iter().nth(target_index).ok_or(TilingError::WindowNotFound(0))?;

    // Focus and raise the target window
    target_window.focus()?;

    // Activate the app to ensure it's in the foreground
    activate_app(pid)?;

    // Focus again after activation
    target_window.focus()
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
