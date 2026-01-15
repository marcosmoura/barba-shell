//! Common test utilities and framework re-exports.
//!
//! This module re-exports the test framework for convenient use in test files.

#![allow(dead_code)]

pub use crate::framework::*;

/// Tolerance in pixels for frame comparisons.
///
/// Window positions can vary slightly due to:
/// - Subpixel rendering
/// - Window manager adjustments
/// - Animation timing
pub const FRAME_TOLERANCE: i32 = 5;

/// Asserts that two frames are approximately equal within the tolerance.
#[macro_export]
macro_rules! assert_frame_approx_eq {
    ($left:expr, $right:expr) => {
        assert_frame_approx_eq!($left, $right, $crate::common::FRAME_TOLERANCE)
    };
    ($left:expr, $right:expr, $tolerance:expr) => {{
        let left = &$left;
        let right = &$right;
        if !left.approx_eq(right, $tolerance) {
            panic!(
                "assertion failed: frames not approximately equal\n  left:  {}\n  right: {}\n  tolerance: {}",
                left, right, $tolerance
            );
        }
    }};
}

/// Asserts that a frame is within the expected tiling area.
#[macro_export]
macro_rules! assert_frame_within {
    ($frame:expr, $area:expr) => {{
        let frame = &$frame;
        let area = &$area;
        assert!(
            frame.x >= area.x - $crate::common::FRAME_TOLERANCE,
            "frame.x ({}) < area.x ({})",
            frame.x,
            area.x
        );
        assert!(
            frame.y >= area.y - $crate::common::FRAME_TOLERANCE,
            "frame.y ({}) < area.y ({})",
            frame.y,
            area.y
        );
        assert!(
            frame.right() <= area.right() + $crate::common::FRAME_TOLERANCE,
            "frame.right() ({}) > area.right() ({})",
            frame.right(),
            area.right()
        );
        assert!(
            frame.bottom() <= area.bottom() + $crate::common::FRAME_TOLERANCE,
            "frame.bottom() ({}) > area.bottom() ({})",
            frame.bottom(),
            area.bottom()
        );
    }};
}
