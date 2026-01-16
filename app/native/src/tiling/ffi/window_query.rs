//! Safe wrappers for SkyLight window query APIs.
//!
#![allow(clippy::doc_markdown)] // Allow SkyLight, CGWindowID, etc. without backticks
//!
//! This module provides efficient batch window enumeration using SkyLight's
//! private APIs. This is significantly faster than querying individual windows
//! via the Accessibility API or CGWindowListCopyWindowInfo.
//!
//! # Performance
//!
//! - SLS Query: ~1-5ms for 50+ windows
//! - CGWindowList: ~5-15ms for 50+ windows
//! - Individual AX queries: ~100-400ms for 50+ windows
//!
//! # Example
//!
//! ```rust,ignore
//! use stache::tiling::ffi::window_query::WindowQuery;
//!
//! // Query all windows on screen
//! if let Some(query) = WindowQuery::all_on_screen() {
//!     for window in query.iter() {
//!         println!("Window {} at {:?}", window.id, window.bounds);
//!     }
//! }
//! ```
//!
//! # Thread Safety
//!
//! Window queries should be performed on the main thread for best results.
//! The query objects are `Send` but not `Sync`.

use std::ffi::c_void;

use super::skylight::{CGRect, get_connection_id};
use crate::tiling::state::Rect;

// ============================================================================
// FFI Declarations
// ============================================================================

type CFTypeRef = *mut c_void;
type CFArrayRef = *const c_void;

// Window query option flags
/// Include windows that are currently visible on screen.
const K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1 << 0;
/// Exclude windows with a window layer of 0 (desktop).
const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;

#[link(name = "SkyLight", kind = "framework")]
unsafe extern "C" {
    // Creates a window query for the specified windows.
    //
    // # Arguments
    //
    // * `cid` - Connection ID
    // * `count` - Number of window IDs to query (0 for all)
    // * `window_ids` - Array of window IDs to query (NULL for all)
    // * `options` - Query options
    //
    // # Returns
    //
    // A query result, or null on failure.
    fn SLSWindowQueryWindows(
        cid: u32,
        window_ids: CFArrayRef,
        count: u32,
        options: u32,
    ) -> CFTypeRef;

    // Creates a window iterator from a query result.
    //
    // # Arguments
    //
    // * `query` - Query result from SLSWindowQueryWindows
    //
    // # Returns
    //
    // An iterator, or null on failure.
    fn SLSWindowQueryResultCopyWindows(query: CFTypeRef) -> CFTypeRef;

    // Advances the iterator to the next window.
    //
    // # Arguments
    //
    // * `iterator` - Window iterator
    //
    // # Returns
    //
    // true if there is a next window, false if iteration is complete.
    fn SLSWindowIteratorAdvance(iterator: CFTypeRef) -> bool;

    // Gets the window ID from the current iterator position.
    fn SLSWindowIteratorGetWindowID(iterator: CFTypeRef) -> u32;

    // Gets the window bounds from the current iterator position.
    fn SLSWindowIteratorGetBounds(iterator: CFTypeRef) -> CGRect;

    // Gets the process ID from the current iterator position.
    fn SLSWindowIteratorGetPID(iterator: CFTypeRef) -> i32;

    // Gets the window level from the current iterator position.
    fn SLSWindowIteratorGetLevel(iterator: CFTypeRef) -> i32;

    // Gets the window tags from the current iterator position.
    fn SLSWindowIteratorGetTags(iterator: CFTypeRef) -> u64;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

// ============================================================================
// Window Info
// ============================================================================

/// Information about a window returned from a query.
#[derive(Debug, Clone)]
pub struct WindowInfo {
    /// Window ID (CGWindowID).
    pub id: u32,
    /// Process ID of the owning application.
    pub pid: i32,
    /// Window bounds in screen coordinates.
    pub bounds: Rect,
    /// Window level (z-order within a layer).
    pub level: i32,
    /// Window tags (bit flags for various properties).
    pub tags: u64,
}

impl WindowInfo {
    /// Returns whether this window is visible.
    ///
    /// Checks the window tags for the visibility flag.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        // Tag bit 0x0010_0000 indicates visible window
        self.tags & 0x0010_0000 != 0
    }

    /// Returns whether this window is minimized.
    #[must_use]
    pub const fn is_minimized(&self) -> bool {
        // Tag bit 0x2 indicates minimized window
        self.tags & 0x2 != 0
    }
}

// ============================================================================
// Window Query
// ============================================================================

/// Efficient batch window query using SkyLight APIs.
///
/// Provides an iterator interface for enumerating windows with their properties.
/// This is much faster than querying individual windows via the Accessibility API.
pub struct WindowQuery {
    /// Query result handle.
    query: CFTypeRef,
    /// Iterator handle.
    iterator: CFTypeRef,
    /// Connection ID.
    _cid: u32,
}

impl WindowQuery {
    /// Queries all windows currently on screen.
    ///
    /// This excludes desktop elements and other system windows.
    ///
    /// # Returns
    ///
    /// A window query, or `None` if the query failed.
    #[must_use]
    pub fn all_on_screen() -> Option<Self> {
        let cid = get_connection_id();
        if cid == 0 {
            return None;
        }

        let options =
            K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS;
        let query = unsafe { SLSWindowQueryWindows(cid, std::ptr::null(), 0, options) };
        if query.is_null() {
            return None;
        }

        let iterator = unsafe { SLSWindowQueryResultCopyWindows(query) };
        if iterator.is_null() {
            unsafe { CFRelease(query.cast_const()) };
            return None;
        }

        Some(Self { query, iterator, _cid: cid })
    }

    /// Queries all windows (including off-screen).
    ///
    /// # Returns
    ///
    /// A window query, or `None` if the query failed.
    #[must_use]
    pub fn all() -> Option<Self> {
        let cid = get_connection_id();
        if cid == 0 {
            return None;
        }

        let options = K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS;
        let query = unsafe { SLSWindowQueryWindows(cid, std::ptr::null(), 0, options) };
        if query.is_null() {
            return None;
        }

        let iterator = unsafe { SLSWindowQueryResultCopyWindows(query) };
        if iterator.is_null() {
            unsafe { CFRelease(query.cast_const()) };
            return None;
        }

        Some(Self { query, iterator, _cid: cid })
    }

    /// Returns an iterator over the queried windows.
    #[must_use]
    pub const fn iter(&self) -> WindowIterator<'_> {
        WindowIterator { query: self, started: false }
    }

    /// Collects all windows into a vector.
    ///
    /// This is a convenience method that consumes the iterator.
    #[must_use]
    pub fn collect_all(&self) -> Vec<WindowInfo> { self.iter().collect() }
}

impl Drop for WindowQuery {
    fn drop(&mut self) {
        if !self.iterator.is_null() {
            unsafe { CFRelease(self.iterator.cast_const()) };
        }
        if !self.query.is_null() {
            unsafe { CFRelease(self.query.cast_const()) };
        }
    }
}

// WindowQuery is Send because SkyLight APIs are thread-safe
unsafe impl Send for WindowQuery {}

impl<'a> IntoIterator for &'a WindowQuery {
    type IntoIter = WindowIterator<'a>;
    type Item = WindowInfo;

    fn into_iter(self) -> Self::IntoIter { self.iter() }
}

// ============================================================================
// Window Iterator
// ============================================================================

/// Iterator over windows from a query.
pub struct WindowIterator<'a> {
    query: &'a WindowQuery,
    started: bool,
}

impl Iterator for WindowIterator<'_> {
    type Item = WindowInfo;

    fn next(&mut self) -> Option<Self::Item> {
        // Advance to next window (or first if not started)
        let has_next = unsafe { SLSWindowIteratorAdvance(self.query.iterator) };
        if !has_next {
            return None;
        }
        self.started = true;

        // Get window properties
        let id = unsafe { SLSWindowIteratorGetWindowID(self.query.iterator) };
        let pid = unsafe { SLSWindowIteratorGetPID(self.query.iterator) };
        let bounds = unsafe { SLSWindowIteratorGetBounds(self.query.iterator) };
        let level = unsafe { SLSWindowIteratorGetLevel(self.query.iterator) };
        let tags = unsafe { SLSWindowIteratorGetTags(self.query.iterator) };

        Some(WindowInfo {
            id,
            pid,
            bounds: bounds.to_rect(),
            level,
            tags,
        })
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_info_visibility_check() {
        let visible = WindowInfo {
            id: 1,
            pid: 100,
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            level: 0,
            tags: 0x100000, // Visible flag
        };
        assert!(visible.is_visible());

        let invisible = WindowInfo {
            id: 2,
            pid: 100,
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            level: 0,
            tags: 0,
        };
        assert!(!invisible.is_visible());
    }

    #[test]
    fn window_info_minimized_check() {
        let minimized = WindowInfo {
            id: 1,
            pid: 100,
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            level: 0,
            tags: 0x2, // Minimized flag
        };
        assert!(minimized.is_minimized());

        let not_minimized = WindowInfo {
            id: 2,
            pid: 100,
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            level: 0,
            tags: 0,
        };
        assert!(!not_minimized.is_minimized());
    }

    #[test]
    fn query_constants_are_correct() {
        assert_eq!(K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY, 1);
        assert_eq!(K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS, 16);
    }
}
