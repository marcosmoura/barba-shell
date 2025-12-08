//! Screen information and enumeration.
//!
//! This module provides functions to query connected displays.

use cocoa::base::{id, nil};
use cocoa::foundation::NSArray;
use core_graphics::display::{
    CGDirectDisplayID, CGDisplay, CGDisplayBounds, CGGetActiveDisplayList, CGMainDisplayID,
};
use objc::{class, msg_send, sel, sel_impl};

use crate::tiling::error::TilingError;
use crate::tiling::state::{Screen, ScreenFrame};

/// Result type for screen operations.
pub type ScreenResult<T> = Result<T, TilingError>;

/// `NSRect` structure matching `AppKit`'s definition.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct NSRect {
    origin: NSPoint,
    size: NSSize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct NSSize {
    width: f64,
    height: f64,
}

/// Gets the list of all connected screens.
pub fn get_all_screens() -> ScreenResult<Vec<Screen>> {
    let display_ids = get_display_ids()?;
    let main_display_id = unsafe { CGMainDisplayID() };

    // Get NSScreen info indexed by display ID for exact matching
    let ns_screen_map = get_ns_screen_map();

    let mut screens = Vec::with_capacity(display_ids.len());

    for display_id in display_ids {
        let _display = CGDisplay::new(display_id);
        let bounds = unsafe { CGDisplayBounds(display_id) };

        let is_main = display_id == main_display_id;

        // Get display name
        let name = if is_main {
            "Main Display".to_string()
        } else {
            format!("Display {display_id}")
        };

        // Convert bounds to our frame type
        #[allow(clippy::cast_possible_truncation)]
        let frame = ScreenFrame {
            x: bounds.origin.x as i32,
            y: bounds.origin.y as i32,
            width: bounds.size.width as u32,
            height: bounds.size.height as u32,
        };

        // Find matching NSScreen by display ID for accurate visible frame
        let usable_frame =
            get_usable_frame_for_display(display_id, &ns_screen_map, &frame, is_main);

        screens.push(Screen {
            id: display_id.to_string(),
            name,
            is_main,
            frame,
            usable_frame,
        });
    }

    // Sort so main display is first
    screens.sort_by(|a, b| b.is_main.cmp(&a.is_main));

    Ok(screens)
}

/// Screen info from `NSScreen`: (`display_id`, `visible_frame`).
struct NSScreenInfo {
    display_id: CGDirectDisplayID,
    visible_frame: NSRect,
}

/// Gets `NSScreen` information mapped by display ID.
fn get_ns_screen_map() -> Vec<NSScreenInfo> {
    let mut result = Vec::new();

    unsafe {
        let screens: id = msg_send![class!(NSScreen), screens];
        if screens == nil {
            return result;
        }

        let count = NSArray::count(screens);
        for i in 0..count {
            let screen: id = msg_send![screens, objectAtIndex: i];
            if screen == nil {
                continue;
            }

            // Get the deviceDescription dictionary
            let device_desc: id = msg_send![screen, deviceDescription];
            if device_desc == nil {
                continue;
            }

            // Get the NSScreenNumber key to get the CGDirectDisplayID
            let screen_number_key: id =
                msg_send![class!(NSString), stringWithUTF8String: b"NSScreenNumber\0".as_ptr()];
            let screen_number: id = msg_send![device_desc, objectForKey: screen_number_key];
            if screen_number == nil {
                continue;
            }

            // NSNumber -> unsigned int (CGDirectDisplayID)
            let display_id: CGDirectDisplayID = msg_send![screen_number, unsignedIntValue];
            let visible_frame: NSRect = msg_send![screen, visibleFrame];

            result.push(NSScreenInfo { display_id, visible_frame });
        }
    }

    result
}

/// Gets the usable frame for a specific display ID.
/// Uses `NSScreen`'s visibleFrame which properly accounts for menu bar and dock.
#[allow(clippy::cast_possible_truncation)]
fn get_usable_frame_for_display(
    display_id: CGDirectDisplayID,
    ns_screens: &[NSScreenInfo],
    cg_frame: &ScreenFrame,
    is_main: bool,
) -> ScreenFrame {
    // Find the matching NSScreen by display ID
    for info in ns_screens {
        if info.display_id == display_id {
            let visible = &info.visible_frame;

            // NSScreen uses bottom-left origin (Cocoa coordinates)
            // CGDisplay uses top-left origin (Quartz coordinates)
            // We need to convert the visibleFrame to Quartz coordinates

            // Get the main screen height to convert coordinates
            let main_screen_height = unsafe {
                let main_screen: id = msg_send![class!(NSScreen), mainScreen];
                let main_frame: NSRect = msg_send![main_screen, frame];
                main_frame.size.height
            };

            // In Cocoa, origin.y is distance from bottom of main screen to bottom of visible area
            // In Quartz, y=0 is at the top of the main screen
            // quartz_y = main_screen_height - cocoa_y - height
            let quartz_y = main_screen_height - visible.origin.y - visible.size.height;

            return ScreenFrame {
                x: visible.origin.x as i32,
                y: quartz_y as i32,
                width: visible.size.width as u32,
                height: visible.size.height as u32,
            };
        }
    }

    // Fallback: estimate menu bar height for main only
    let menu_bar_height = if is_main { 25 } else { 0 };
    ScreenFrame {
        x: cg_frame.x,
        y: cg_frame.y + menu_bar_height,
        width: cg_frame.width,
        height: cg_frame.height.saturating_sub(menu_bar_height as u32),
    }
}

/// Gets raw display IDs from Core Graphics.
fn get_display_ids() -> ScreenResult<Vec<CGDirectDisplayID>> {
    // First call to get count
    let mut display_count: u32 = 0;
    let result = unsafe { CGGetActiveDisplayList(0, std::ptr::null_mut(), &raw mut display_count) };

    if result != 0 {
        return Err(TilingError::OperationFailed(format!(
            "Failed to get display count: {result}"
        )));
    }

    if display_count == 0 {
        return Ok(Vec::new());
    }

    // Second call to get display IDs
    let mut display_ids = vec![0u32; display_count as usize];
    let result = unsafe {
        CGGetActiveDisplayList(display_count, display_ids.as_mut_ptr(), &raw mut display_count)
    };

    if result != 0 {
        return Err(TilingError::OperationFailed(format!(
            "Failed to get display list: {result}"
        )));
    }

    Ok(display_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_all_screens() {
        // This test requires a display, skip in headless CI
        if std::env::var("CI").is_ok() {
            return;
        }

        let screens = get_all_screens().unwrap();
        assert!(!screens.is_empty());
        assert!(screens.iter().any(|s| s.is_main));
    }
}
