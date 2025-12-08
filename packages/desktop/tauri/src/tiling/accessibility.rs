//! macOS Accessibility API bindings.
//!
//! This module provides safe Rust wrappers around the macOS Accessibility API
//! for window manipulation and observation.

use std::ffi::c_void;
use std::ptr;

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::display::CGPoint;
use core_graphics::geometry::CGSize;

use super::error::TilingError;
use super::state::WindowFrame;

/// Result type for accessibility operations.
pub type AXResult<T> = Result<T, TilingError>;

// Foreign function declarations for Accessibility API
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> i32;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFStringRef) -> i32;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> i32;
    fn AXIsProcessTrusted() -> bool;
    fn AXValueCreate(value_type: u32, value: *const c_void) -> CFTypeRef;
    fn AXValueGetValue(value: CFTypeRef, value_type: u32, value_out: *mut c_void) -> bool;
    fn CFRelease(cf: CFTypeRef);
}

// Opaque types
type AXUIElementRef = *mut c_void;
type CFTypeRef = *mut c_void;

// AXValue types
const AX_VALUE_TYPE_CGPOINT: u32 = 1;
const AX_VALUE_TYPE_CGSIZE: u32 = 2;

// AX error codes
const AX_ERROR_SUCCESS: i32 = 0;
const AX_ERROR_FAILURE: i32 = -25200;
const AX_ERROR_ILLEGAL_ARGUMENT: i32 = -25201;
const AX_ERROR_INVALID_UIELEMENT: i32 = -25202;
const AX_ERROR_INVALID_UIELEMENT_OBSERVER: i32 = -25203;
const AX_ERROR_CANNOT_COMPLETE: i32 = -25204;
const AX_ERROR_ATTRIBUTE_UNSUPPORTED: i32 = -25205;
const AX_ERROR_ACTION_UNSUPPORTED: i32 = -25206;
const AX_ERROR_NOTIFICATION_UNSUPPORTED: i32 = -25207;
const AX_ERROR_NOT_IMPLEMENTED: i32 = -25208;
const AX_ERROR_NOTIFICATION_ALREADY_REGISTERED: i32 = -25209;
const AX_ERROR_NOTIFICATION_NOT_REGISTERED: i32 = -25210;
const AX_ERROR_API_DISABLED: i32 = -25211;
const AX_ERROR_NO_VALUE: i32 = -25212;

/// Attribute names for accessibility elements.
pub mod attributes {
    pub const WINDOWS: &str = "AXWindows";
    pub const FOCUSED_WINDOW: &str = "AXFocusedWindow";
    pub const SUBROLE: &str = "AXSubrole";
    pub const POSITION: &str = "AXPosition";
    pub const SIZE: &str = "AXSize";
    pub const MAIN: &str = "AXMain";
    /// Application hidden state (like Cmd+H).
    pub const HIDDEN: &str = "AXHidden";
    /// System-wide focused application.
    pub const FOCUSED_APPLICATION: &str = "AXFocusedApplication";
}

/// Action names for accessibility elements.
pub mod actions {
    pub const RAISE: &str = "AXRaise";
}

/// Converts an AX error code to a `TilingError`.
fn ax_error_to_result(code: i32) -> AXResult<()> {
    match code {
        AX_ERROR_SUCCESS => Ok(()),
        AX_ERROR_API_DISABLED => Err(TilingError::AccessibilityNotAuthorized),
        AX_ERROR_INVALID_UIELEMENT | AX_ERROR_INVALID_UIELEMENT_OBSERVER => {
            Err(TilingError::WindowNotFound(0))
        }
        AX_ERROR_ATTRIBUTE_UNSUPPORTED | AX_ERROR_NO_VALUE => Err(TilingError::OperationFailed(
            "Attribute not supported".to_string(),
        )),
        AX_ERROR_ACTION_UNSUPPORTED => {
            Err(TilingError::OperationFailed("Action not supported".to_string()))
        }
        AX_ERROR_CANNOT_COMPLETE => Err(TilingError::OperationFailed(
            "Cannot complete operation".to_string(),
        )),
        AX_ERROR_FAILURE => Err(TilingError::OperationFailed("General AX failure".to_string())),
        AX_ERROR_ILLEGAL_ARGUMENT => {
            Err(TilingError::OperationFailed("Illegal argument".to_string()))
        }
        AX_ERROR_NOTIFICATION_UNSUPPORTED
        | AX_ERROR_NOT_IMPLEMENTED
        | AX_ERROR_NOTIFICATION_ALREADY_REGISTERED
        | AX_ERROR_NOTIFICATION_NOT_REGISTERED => {
            Err(TilingError::OperationFailed("Notification error".to_string()))
        }
        _ => Err(TilingError::OperationFailed(format!("Unknown AX error: {code}"))),
    }
}

/// Checks if the application has accessibility permissions.
#[must_use]
pub fn is_accessibility_enabled() -> bool { unsafe { AXIsProcessTrusted() } }

/// A wrapper around an `AXUIElement`.
#[derive(Debug)]
pub struct AccessibilityElement {
    element: AXUIElementRef,
}

impl Drop for AccessibilityElement {
    fn drop(&mut self) {
        if !self.element.is_null() {
            unsafe { CFRelease(self.element.cast()) };
        }
    }
}

// SAFETY: AXUIElementRef is thread-safe according to Apple documentation.
// The Accessibility API can be called from any thread.
unsafe impl Send for AccessibilityElement {}
unsafe impl Sync for AccessibilityElement {}

impl AccessibilityElement {
    /// Creates a system-wide accessibility element.
    #[must_use]
    pub fn system_wide() -> Self {
        Self {
            element: unsafe { AXUIElementCreateSystemWide() },
        }
    }

    /// Creates an accessibility element for an application.
    #[must_use]
    pub fn application(pid: i32) -> Self {
        Self {
            element: unsafe { AXUIElementCreateApplication(pid) },
        }
    }

    /// Creates from a raw element reference (takes ownership).
    ///
    /// # Safety
    /// The caller must ensure that `element` is a valid `AXUIElementRef`.
    #[must_use]
    pub const unsafe fn from_raw(element: AXUIElementRef) -> Self { Self { element } }

    /// Gets the process ID of this element's application.
    pub fn pid(&self) -> AXResult<i32> {
        let mut pid: i32 = 0;
        let result = unsafe { AXUIElementGetPid(self.element, &raw mut pid) };
        ax_error_to_result(result)?;
        Ok(pid)
    }

    /// Gets a string attribute value.
    pub fn get_string_attribute(&self, attribute: &str) -> AXResult<String> {
        let attr_cf = CFString::new(attribute);
        let mut value: CFTypeRef = ptr::null_mut();

        let result = unsafe {
            AXUIElementCopyAttributeValue(
                self.element,
                attr_cf.as_concrete_TypeRef(),
                &raw mut value,
            )
        };

        ax_error_to_result(result)?;

        if value.is_null() {
            return Err(TilingError::OperationFailed("Null value returned".to_string()));
        }

        // SAFETY: We checked that value is not null and the API guarantees it's a CFString
        let cf_string: CFString = unsafe { CFString::wrap_under_get_rule(value.cast()) };
        Ok(cf_string.to_string())
    }

    /// Gets the position of this element.
    pub fn get_position(&self) -> AXResult<(f64, f64)> {
        let attr_cf = CFString::new(attributes::POSITION);
        let mut value: CFTypeRef = ptr::null_mut();

        let result = unsafe {
            AXUIElementCopyAttributeValue(
                self.element,
                attr_cf.as_concrete_TypeRef(),
                &raw mut value,
            )
        };

        ax_error_to_result(result)?;

        if value.is_null() {
            return Err(TilingError::OperationFailed("Null value returned".to_string()));
        }

        let mut point = CGPoint::new(0.0, 0.0);
        let success = unsafe {
            AXValueGetValue(value, AX_VALUE_TYPE_CGPOINT, ptr::from_mut(&mut point).cast())
        };

        unsafe { CFRelease(value) };

        if success {
            Ok((point.x, point.y))
        } else {
            Err(TilingError::OperationFailed(
                "Failed to extract position".to_string(),
            ))
        }
    }

    /// Gets the size of this element.
    pub fn get_size(&self) -> AXResult<(f64, f64)> {
        let attr_cf = CFString::new(attributes::SIZE);
        let mut value: CFTypeRef = ptr::null_mut();

        let result = unsafe {
            AXUIElementCopyAttributeValue(
                self.element,
                attr_cf.as_concrete_TypeRef(),
                &raw mut value,
            )
        };

        ax_error_to_result(result)?;

        if value.is_null() {
            return Err(TilingError::OperationFailed("Null value returned".to_string()));
        }

        let mut size = CGSize::new(0.0, 0.0);
        let success = unsafe {
            AXValueGetValue(value, AX_VALUE_TYPE_CGSIZE, ptr::from_mut(&mut size).cast())
        };

        unsafe { CFRelease(value) };

        if success {
            Ok((size.width, size.height))
        } else {
            Err(TilingError::OperationFailed(
                "Failed to extract size".to_string(),
            ))
        }
    }

    /// Gets the frame (position and size) of this element.
    pub fn get_frame(&self) -> AXResult<WindowFrame> {
        let (x, y) = self.get_position()?;
        let (width, height) = self.get_size()?;

        // Convert to i32/u32, clamping to valid ranges
        #[allow(clippy::cast_possible_truncation)]
        Ok(WindowFrame {
            x: x as i32,
            y: y as i32,
            width: width.max(0.0) as u32,
            height: height.max(0.0) as u32,
        })
    }

    /// Sets the position of this element.
    pub fn set_position(&self, x: f64, y: f64) -> AXResult<()> {
        let point = CGPoint::new(x, y);
        let value = unsafe { AXValueCreate(AX_VALUE_TYPE_CGPOINT, ptr::from_ref(&point).cast()) };

        if value.is_null() {
            return Err(TilingError::OperationFailed(
                "Failed to create position value".to_string(),
            ));
        }

        let attr_cf = CFString::new(attributes::POSITION);
        let result = unsafe {
            AXUIElementSetAttributeValue(self.element, attr_cf.as_concrete_TypeRef(), value)
        };

        unsafe { CFRelease(value) };

        ax_error_to_result(result)
    }

    /// Sets the size of this element.
    pub fn set_size(&self, width: f64, height: f64) -> AXResult<()> {
        let size = CGSize::new(width, height);
        let value = unsafe { AXValueCreate(AX_VALUE_TYPE_CGSIZE, ptr::from_ref(&size).cast()) };

        if value.is_null() {
            return Err(TilingError::OperationFailed(
                "Failed to create size value".to_string(),
            ));
        }

        let attr_cf = CFString::new(attributes::SIZE);
        let result = unsafe {
            AXUIElementSetAttributeValue(self.element, attr_cf.as_concrete_TypeRef(), value)
        };

        unsafe { CFRelease(value) };

        ax_error_to_result(result)
    }

    /// Sets the frame (position and size) of this element.
    pub fn set_frame(&self, frame: &WindowFrame) -> AXResult<()> {
        // First set size, then position - this order works better for some apps
        // because reducing size first ensures the window can fit at the target position
        self.set_size(f64::from(frame.width), f64::from(frame.height))?;
        self.set_position(f64::from(frame.x), f64::from(frame.y))?;
        // Set size again after position in case the window was constrained
        self.set_size(f64::from(frame.width), f64::from(frame.height))?;
        Ok(())
    }

    /// Sets a boolean attribute value.
    pub fn set_bool_attribute(&self, attribute: &str, value: bool) -> AXResult<()> {
        let attr_cf = CFString::new(attribute);
        let cf_bool = if value {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };

        let result = unsafe {
            AXUIElementSetAttributeValue(
                self.element,
                attr_cf.as_concrete_TypeRef(),
                cf_bool.as_concrete_TypeRef().cast::<c_void>().cast_mut(),
            )
        };

        ax_error_to_result(result)
    }

    /// Performs an action on this element.
    pub fn perform_action(&self, action: &str) -> AXResult<()> {
        let action_cf = CFString::new(action);
        let result =
            unsafe { AXUIElementPerformAction(self.element, action_cf.as_concrete_TypeRef()) };
        ax_error_to_result(result)
    }

    /// Raises this window to the front.
    pub fn raise(&self) -> AXResult<()> { self.perform_action(actions::RAISE) }

    /// Focuses this window.
    pub fn focus(&self) -> AXResult<()> {
        self.set_bool_attribute(attributes::MAIN, true)?;
        self.raise()
    }

    /// Gets an element attribute (like focused application).
    pub fn get_element_attribute(&self, attribute: &str) -> AXResult<Self> {
        let attr_cf = CFString::new(attribute);
        let mut value: CFTypeRef = ptr::null_mut();

        let result = unsafe {
            AXUIElementCopyAttributeValue(
                self.element,
                attr_cf.as_concrete_TypeRef(),
                &raw mut value,
            )
        };

        ax_error_to_result(result)?;

        if value.is_null() {
            return Err(TilingError::OperationFailed("Null element returned".to_string()));
        }

        // SAFETY: We take ownership of the returned element
        Ok(unsafe { Self::from_raw(value) })
    }

    /// Gets the focused window of an application element.
    pub fn get_focused_window(&self) -> AXResult<Self> {
        self.get_element_attribute(attributes::FOCUSED_WINDOW)
            .map_err(|_| TilingError::WindowNotFound(0))
    }

    /// Gets all windows of an application element.
    pub fn get_windows(&self) -> AXResult<Vec<Self>> {
        let attr_cf = CFString::new(attributes::WINDOWS);
        let mut value: CFTypeRef = ptr::null_mut();

        let result = unsafe {
            AXUIElementCopyAttributeValue(
                self.element,
                attr_cf.as_concrete_TypeRef(),
                &raw mut value,
            )
        };

        ax_error_to_result(result)?;

        if value.is_null() {
            return Ok(Vec::new());
        }

        // The value is a CFArray of AXUIElements
        // We need to iterate through it and create AccessibilityElement for each
        let array: core_foundation::array::CFArray<CFType> =
            unsafe { core_foundation::array::CFArray::wrap_under_get_rule(value.cast()) };

        let mut windows = Vec::with_capacity(array.len() as usize);

        for i in 0..array.len() {
            if let Some(elem) = array.get(i) {
                // Retain the element since we're taking ownership
                let raw: *mut c_void = elem.as_concrete_TypeRef().cast::<c_void>().cast_mut();
                unsafe { core_foundation::base::CFRetain(raw.cast()) };
                windows.push(unsafe { Self::from_raw(raw) });
            }
        }

        Ok(windows)
    }

    /// Gets the subrole of this element (e.g., AXDialog, AXFloatingWindow, AXSheet).
    pub fn get_subrole(&self) -> Option<String> {
        self.get_string_attribute(attributes::SUBROLE).ok()
    }

    /// Checks if this element is a dialog, sheet, or other non-tileable window type.
    ///
    /// Returns `true` for:
    /// - Dialogs (`AXDialog`)
    /// - Sheets (`AXSheet`) - slide-down panels attached to windows
    /// - System dialogs (`AXSystemDialog`)
    /// - Floating windows (`AXFloatingWindow`) - palettes, inspectors
    #[must_use]
    pub fn is_dialog_or_sheet(&self) -> bool {
        // Window subroles that should NOT be tiled
        const NON_TILEABLE_SUBROLES: &[&str] =
            &["AXDialog", "AXSheet", "AXSystemDialog", "AXFloatingWindow"];

        self.get_subrole()
            .is_some_and(|subrole| NON_TILEABLE_SUBROLES.iter().any(|&s| subrole == s))
    }
}
