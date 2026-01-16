//! Safe wrappers for macOS `SkyLight` private framework APIs.
//!
#![allow(clippy::doc_markdown)] // Allow SkyLight, CGRect, etc. without backticks
//!
//! This module provides safe Rust wrappers around the `SkyLight` private framework,
//! which offers low-level window server operations. These APIs enable performance
//! optimizations that are not possible with the public Accessibility API.
//!
//! # Screen Update Batching
//!
//! The primary feature is screen update batching via `SLSDisableUpdate`/`SLSReenableUpdate`.
//! This prevents the window server from refreshing the screen during batch operations,
//! reducing visual tearing and improving frame timing consistency.
//!
//! ```rust,ignore
//! use stache::tiling::ffi::skylight::UpdateGuard;
//!
//! // Updates are batched while the guard is held
//! let _guard = UpdateGuard::new();
//! // ... move multiple windows ...
//! // Screen refreshes when guard is dropped
//! ```
//!
//! # Safety
//!
//! These APIs are private and undocumented. While they work without SIP disabled,
//! they may change between macOS versions. All unsafe code is encapsulated in this
//! module with safe wrappers.
//!
//! # Thread Safety
//!
//! The SkyLight APIs are thread-safe. The connection ID is cached and can be
//! accessed from any thread. `UpdateGuard` is `Send` but not `Sync` (should
//! be created and dropped on the same thread as the window operations).

use std::ffi::c_void;
use std::sync::OnceLock;

use crate::tiling::state::Rect;

// ============================================================================
// FFI Type Definitions
// ============================================================================

/// CGRect structure for window bounds.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

/// CGPoint structure for origin coordinates.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

/// CGSize structure for width and height.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

impl CGRect {
    /// Converts this CGRect to our internal Rect type.
    #[must_use]
    pub const fn to_rect(self) -> Rect {
        Rect {
            x: self.origin.x,
            y: self.origin.y,
            width: self.size.width,
            height: self.size.height,
        }
    }
}

// ============================================================================
// FFI Declarations
// ============================================================================

#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C" {
    /// Returns the main connection ID to the window server.
    ///
    /// This ID is required for all SkyLight operations. It remains valid
    /// for the lifetime of the process.
    ///
    /// Note: Uses `u32` to match the existing declaration in `utils/window.rs`.
    fn SLSMainConnectionID() -> u32;

    /// Disables screen updates for the given connection.
    ///
    /// While disabled, window operations are queued but not rendered.
    /// Must be balanced with `SLSReenableUpdate`. Nested calls are supported.
    ///
    /// # Warning
    ///
    /// Holding updates disabled for too long (>16ms at 60Hz) may cause
    /// visual glitches. Always use `UpdateGuard` to ensure proper cleanup.
    fn SLSDisableUpdate(cid: u32) -> i32;

    /// Re-enables screen updates for the given connection.
    ///
    /// Triggers a screen refresh with all queued window operations applied
    /// atomically. Must be called once for each `SLSDisableUpdate` call.
    fn SLSReenableUpdate(cid: u32) -> i32;

    /// Gets the bounds of a window from the window server.
    ///
    /// This is much faster than querying via the Accessibility API (~0.1-0.5ms vs ~2-8ms).
    ///
    /// # Arguments
    ///
    /// * `cid` - Connection ID from `SLSMainConnectionID()`
    /// * `wid` - Window ID (CGWindowID)
    /// * `bounds` - Output parameter for the window's frame
    ///
    /// # Returns
    ///
    /// 0 on success, non-zero error code on failure.
    fn CGSGetWindowBounds(cid: u32, wid: u32, bounds: *mut CGRect) -> i32;
}

// Private Accessibility API to get the window server ID from an AXUIElement.
// This is linked from the ApplicationServices framework, not SkyLight.
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    // Gets the CGWindowID from an AXUIElement.
    //
    // This allows mapping from an AXUIElement (used for window manipulation)
    // to a CGWindowID (used for fast window server queries).
    //
    // # Arguments
    //
    // * `element` - An AXUIElementRef representing a window
    // * `wid` - Output parameter for the CGWindowID
    //
    // # Returns
    //
    // `kAXErrorSuccess` (0) on success, or an AXError code on failure.
    //
    // Note: Uses `*mut c_void` to match the existing declaration in `window.rs`.
    fn _AXUIElementGetWindow(element: *mut c_void, wid: *mut u32) -> i32;
}

// ============================================================================
// Connection ID Cache
// ============================================================================

/// Cached connection ID to the window server.
///
/// Initialized on first use via `SLSMainConnectionID()`.
static CONNECTION_ID: OnceLock<u32> = OnceLock::new();

/// Returns the cached window server connection ID.
///
/// The connection ID is obtained once and cached for the lifetime of the process.
/// Returns 0 if the connection could not be established (should not happen in
/// normal operation).
#[inline]
pub fn get_connection_id() -> u32 {
    *CONNECTION_ID.get_or_init(|| {
        let cid = unsafe { SLSMainConnectionID() };
        if cid == 0 {
            eprintln!("stache: skylight: failed to get connection ID");
        }
        cid
    })
}

// ============================================================================
// Screen Update Control
// ============================================================================

/// Disables screen updates for batch window operations.
///
/// Returns `true` if updates were successfully disabled.
///
/// # Warning
///
/// This is a low-level API. Prefer using `UpdateGuard` for RAII-based
/// management that guarantees reenabling even on early returns or panics.
#[inline]
#[must_use]
pub fn disable_update() -> bool {
    let cid = get_connection_id();
    if cid == 0 {
        return false;
    }
    let result = unsafe { SLSDisableUpdate(cid) };
    result == 0
}

/// Re-enables screen updates after batch window operations.
///
/// Returns `true` if updates were successfully re-enabled.
#[inline]
#[must_use]
pub fn reenable_update() -> bool {
    let cid = get_connection_id();
    if cid == 0 {
        return false;
    }
    let result = unsafe { SLSReenableUpdate(cid) };
    result == 0
}

// ============================================================================
// Fast Window Bounds Query
// ============================================================================

/// Gets window bounds directly from the window server.
///
/// This is significantly faster than querying via the Accessibility API:
/// - CGS: ~0.1-0.5ms per call
/// - AX: ~2-8ms per call
///
/// # Arguments
///
/// * `window_id` - The CGWindowID of the window
///
/// # Returns
///
/// The window's bounds as a `Rect`, or `None` if the query failed
/// (e.g., window closed, invalid ID).
///
/// # Example
///
/// ```rust,ignore
/// if let Some(bounds) = get_window_bounds_fast(window_id) {
///     println!("Window at ({}, {}), size {}x{}",
///         bounds.x, bounds.y, bounds.width, bounds.height);
/// }
/// ```
#[must_use]
pub fn get_window_bounds_fast(window_id: u32) -> Option<Rect> {
    let cid = get_connection_id();
    if cid == 0 {
        return None;
    }

    let mut bounds = CGRect::default();
    let result = unsafe { CGSGetWindowBounds(cid, window_id, &raw mut bounds) };

    if result == 0 {
        Some(bounds.to_rect())
    } else {
        None
    }
}

// ============================================================================
// AXUIElement to CGWindowID Mapping
// ============================================================================

/// Gets the CGWindowID from an AXUIElement reference.
///
/// This allows mapping from an accessibility element (used for window manipulation)
/// to a window server ID (used for fast bounds queries).
///
/// # Arguments
///
/// * `ax_element` - A raw pointer to an AXUIElementRef
///
/// # Returns
///
/// The CGWindowID, or `None` if the element is not a window or the query failed.
///
/// # Safety
///
/// The caller must ensure that `ax_element` is a valid AXUIElementRef pointer.
/// This function is safe to call with a null pointer (returns `None`).
#[must_use]
#[allow(clippy::not_unsafe_ptr_arg_deref)] // Null-checked before use
pub fn get_window_id_from_ax(ax_element: *mut c_void) -> Option<u32> {
    if ax_element.is_null() {
        return None;
    }

    let mut window_id: u32 = 0;
    let result = unsafe { _AXUIElementGetWindow(ax_element, &raw mut window_id) };

    // kAXErrorSuccess = 0
    if result == 0 && window_id != 0 {
        Some(window_id)
    } else {
        None
    }
}

// ============================================================================
// RAII Update Guard
// ============================================================================

/// RAII guard that disables screen updates while held.
///
/// Screen updates are disabled when the guard is created and automatically
/// re-enabled when the guard is dropped. This ensures proper cleanup even
/// in the presence of early returns or panics.
///
/// # Example
///
/// ```rust,ignore
/// use stache::tiling::ffi::skylight::UpdateGuard;
///
/// fn animate_batch(windows: &[Window]) {
///     // Disable screen updates
///     let _guard = UpdateGuard::new();
///     
///     // All window moves happen without screen refresh
///     for window in windows {
///         window.move_to(target);
///     }
///     
///     // Screen refreshes here when guard drops
/// }
/// ```
///
/// # Performance Notes
///
/// - Keep the guard scope as short as possible
/// - Maximum recommended hold time: 16ms (1 frame at 60Hz)
/// - Nested guards are supported but add overhead
#[derive(Debug)]
pub struct UpdateGuard {
    /// Whether updates were successfully disabled.
    ///
    /// If `false`, no reenable call will be made on drop.
    disabled: bool,
}

impl UpdateGuard {
    /// Creates a new update guard, disabling screen updates.
    ///
    /// Returns `None` if updates could not be disabled (e.g., invalid
    /// connection ID). In this case, operations proceed without batching.
    #[must_use]
    pub fn new() -> Option<Self> {
        if disable_update() {
            Some(Self { disabled: true })
        } else {
            None
        }
    }

    /// Creates a guard that conditionally disables updates.
    ///
    /// If `enabled` is `false`, returns a no-op guard that doesn't
    /// disable updates. This is useful for feature flags.
    #[must_use]
    pub fn new_if(enabled: bool) -> Option<Self> { if enabled { Self::new() } else { None } }
}

impl Drop for UpdateGuard {
    fn drop(&mut self) {
        if self.disabled {
            let _ = reenable_update();
        }
    }
}

// UpdateGuard is Send because SkyLight APIs are thread-safe,
// but not Sync because the disable/reenable should happen on the
// same thread as the window operations for best results.
unsafe impl Send for UpdateGuard {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::*;

    #[test]
    fn connection_id_is_cached() {
        // First call initializes
        let cid1 = get_connection_id();
        // Second call returns cached value
        let cid2 = get_connection_id();

        assert_eq!(cid1, cid2);
        // On a real macOS system, cid should be non-zero
        // In CI without a display, it might be 0
    }

    #[test]
    fn update_guard_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<UpdateGuard>();
    }

    #[test]
    fn update_guard_new_if_disabled_returns_none() {
        let guard = UpdateGuard::new_if(false);
        assert!(guard.is_none());
    }

    #[test]
    fn update_guard_drop_is_safe_when_disabled_false() {
        // Test that dropping a guard with disabled=false doesn't crash
        let guard = UpdateGuard { disabled: false };
        drop(guard);
    }

    #[test]
    fn cg_rect_to_rect_conversion() {
        let cg_rect = CGRect {
            origin: CGPoint { x: 10.0, y: 20.0 },
            size: CGSize { width: 100.0, height: 200.0 },
        };
        let rect = cg_rect.to_rect();

        assert!((rect.x - 10.0).abs() < f64::EPSILON);
        assert!((rect.y - 20.0).abs() < f64::EPSILON);
        assert!((rect.width - 100.0).abs() < f64::EPSILON);
        assert!((rect.height - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn get_window_id_from_ax_with_null_returns_none() {
        let result = get_window_id_from_ax(ptr::null_mut());
        assert!(result.is_none());
    }

    #[test]
    fn get_window_bounds_fast_with_invalid_id_returns_none() {
        // Window ID 0 should be invalid
        let result = get_window_bounds_fast(0);
        assert!(result.is_none());

        // Very high window ID should also be invalid (unless system has that many windows)
        let result = get_window_bounds_fast(u32::MAX);
        assert!(result.is_none());
    }
}
