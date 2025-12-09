//! CVDisplayLink-based animation timing for smooth, display-synced animations.
//!
//! This module provides a macOS-specific animation timer that synchronizes with
//! the display refresh rate using `CVDisplayLink`. This eliminates jank caused by
//! `thread::sleep` not aligning with vertical sync.

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Opaque type for `CVDisplayLink`.
#[repr(C)]
struct CVDisplayLink {
    _private: [u8; 0],
}

/// Opaque type for `CVDisplayLinkRef`.
type CVDisplayLinkRef = *mut CVDisplayLink;

/// Return type for Core Video functions.
type CVReturn = i32;

/// Success return value.
const K_CV_RETURN_SUCCESS: CVReturn = 0;

/// Output time structure for display link callback.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct CVTimeStamp {
    version: u32,
    video_time_scale: i32,
    video_time: i64,
    host_time: u64,
    rate_scalar: f64,
    video_refresh_period: i64,
    smpte_time: SMPTETime,
    flags: u64,
    reserved: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct SMPTETime {
    subframes: i16,
    subframe_divisor: i16,
    counter: u32,
    time_type: u32,
    flags: u32,
    hours: i16,
    minutes: i16,
    seconds: i16,
    frames: i16,
}

/// Callback type for `CVDisplayLink`.
type CVDisplayLinkOutputCallback = extern "C" fn(
    display_link: CVDisplayLinkRef,
    in_now: *const CVTimeStamp,
    in_output_time: *const CVTimeStamp,
    flags_in: u64,
    flags_out: *mut u64,
    display_link_context: *mut c_void,
) -> CVReturn;

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVDisplayLinkCreateWithActiveCGDisplays(display_link_out: *mut CVDisplayLinkRef)
    -> CVReturn;
    fn CVDisplayLinkSetOutputCallback(
        display_link: CVDisplayLinkRef,
        callback: CVDisplayLinkOutputCallback,
        user_info: *mut c_void,
    ) -> CVReturn;
    fn CVDisplayLinkStart(display_link: CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkStop(display_link: CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkRelease(display_link: CVDisplayLinkRef);
}

/// Context passed to the display link callback.
struct DisplayLinkContext {
    /// Callback to invoke on each frame.
    callback: Box<dyn Fn() -> bool + Send + Sync>,
    /// Flag to signal stopping.
    should_stop: Arc<AtomicBool>,
}

/// Manages a `CVDisplayLink` for display-synced animation callbacks.
pub struct DisplayLink {
    /// The native display link reference.
    link: CVDisplayLinkRef,
    /// Context holding the callback.
    context: *mut DisplayLinkContext,
    /// Whether the display link is running.
    running: Arc<AtomicBool>,
}

// Safety: DisplayLink manages its own synchronization
unsafe impl Send for DisplayLink {}
unsafe impl Sync for DisplayLink {}

impl DisplayLink {
    /// Creates a new display link.
    ///
    /// Returns `None` if the display link cannot be created.
    #[must_use]
    pub fn new() -> Option<Self> {
        let mut link: CVDisplayLinkRef = std::ptr::null_mut();

        // Safety: We're calling a C function that initializes the pointer
        let result = unsafe { CVDisplayLinkCreateWithActiveCGDisplays(&raw mut link) };

        if result != K_CV_RETURN_SUCCESS || link.is_null() {
            return None;
        }

        Some(Self {
            link,
            context: std::ptr::null_mut(),
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Starts the display link with a callback.
    ///
    /// The callback is invoked on each display refresh. Return `true` to continue,
    /// or `false` to stop the display link.
    pub fn start<F>(&mut self, callback: F) -> bool
    where F: Fn() -> bool + Send + Sync + 'static {
        if self.running.load(Ordering::SeqCst) {
            return false;
        }

        let should_stop = Arc::new(AtomicBool::new(false));

        let context = Box::new(DisplayLinkContext {
            callback: Box::new(callback),
            should_stop: Arc::clone(&should_stop),
        });

        self.context = Box::into_raw(context);

        // Safety: Setting up the callback with our context
        let result = unsafe {
            CVDisplayLinkSetOutputCallback(self.link, display_link_callback, self.context.cast())
        };

        if result != K_CV_RETURN_SUCCESS {
            // Clean up the context
            unsafe {
                drop(Box::from_raw(self.context));
            }
            self.context = std::ptr::null_mut();
            return false;
        }

        // Safety: Starting the display link
        let result = unsafe { CVDisplayLinkStart(self.link) };

        if result != K_CV_RETURN_SUCCESS {
            // Clean up the context
            unsafe {
                drop(Box::from_raw(self.context));
            }
            self.context = std::ptr::null_mut();
            return false;
        }

        self.running.store(true, Ordering::SeqCst);
        true
    }

    /// Stops the display link.
    pub fn stop(&mut self) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }

        // Signal the callback to stop
        if !self.context.is_null() {
            // Safety: We know the context is valid while running
            unsafe {
                (*self.context).should_stop.store(true, Ordering::SeqCst);
            }
        }

        // Safety: Stopping the display link
        unsafe {
            CVDisplayLinkStop(self.link);
        }

        // Clean up the context
        if !self.context.is_null() {
            // Safety: We're done with the context
            unsafe {
                drop(Box::from_raw(self.context));
            }
            self.context = std::ptr::null_mut();
        }

        self.running.store(false, Ordering::SeqCst);
    }

    /// Returns whether the display link is currently running.
    #[cfg(test)]
    #[must_use]
    pub fn is_running(&self) -> bool { self.running.load(Ordering::SeqCst) }
}

impl Drop for DisplayLink {
    fn drop(&mut self) {
        self.stop();

        // Safety: Releasing the display link
        if !self.link.is_null() {
            unsafe {
                CVDisplayLinkRelease(self.link);
            }
        }
    }
}

/// The callback invoked by `CVDisplayLink` on each display refresh.
extern "C" fn display_link_callback(
    _display_link: CVDisplayLinkRef,
    _in_now: *const CVTimeStamp,
    _in_output_time: *const CVTimeStamp,
    _flags_in: u64,
    _flags_out: *mut u64,
    display_link_context: *mut c_void,
) -> CVReturn {
    if display_link_context.is_null() {
        return K_CV_RETURN_SUCCESS;
    }

    // Safety: We know the context is valid while the display link is running
    let context = unsafe { &*(display_link_context as *const DisplayLinkContext) };

    // Check if we should stop
    if context.should_stop.load(Ordering::SeqCst) {
        return K_CV_RETURN_SUCCESS;
    }

    // Call the user callback
    let should_continue = (context.callback)();

    if !should_continue {
        context.should_stop.store(true, Ordering::SeqCst);
    }

    K_CV_RETURN_SUCCESS
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU32;
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_display_link_creation() {
        let display_link = DisplayLink::new();
        assert!(display_link.is_some(), "Should be able to create display link");
    }

    #[test]
    fn test_display_link_start_stop() {
        let mut display_link = DisplayLink::new().expect("Failed to create display link");

        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        let started = display_link.start(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
            // Stop after a few frames
            counter_clone.load(Ordering::SeqCst) < 5
        });

        assert!(started, "Display link should start");

        // Wait a bit for some callbacks
        std::thread::sleep(Duration::from_millis(100));

        display_link.stop();

        assert!(!display_link.is_running(), "Display link should be stopped");
        assert!(
            counter.load(Ordering::SeqCst) > 0,
            "Callback should have been called"
        );
    }
}
